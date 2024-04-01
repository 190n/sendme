use askama::Template;
use axum::{
	extract::{multipart::Field, Multipart},
	http::{
		header::{HeaderMap, CONTENT_LENGTH},
		StatusCode,
	},
	response, Extension,
};
use std::{
	ffi::OsStr,
	fmt::Debug,
	fs::{DirBuilder, File},
	io::{self, Stdout, Write},
	path::Path,
};

use crate::{
	args::{Mode, Output},
	progress::Progress,
	State,
};

#[derive(Template, Clone, Copy)]
#[template(path = "upload-success.html")]
pub struct UploadSuccessTemplate {
	message: &'static str,
	keep_running: bool,
}

enum FileOrStdout {
	File(File),
	Stdout(Stdout),
}

impl FileOrStdout {
	pub fn flush(&mut self) -> io::Result<()> {
		match self {
			FileOrStdout::Stdout(s) => s.flush(),
			_ => Ok(()),
		}
	}

	pub fn from_mode(mode: &Mode, field: &Field<'_>) -> response::Result<Self> {
		Ok(match mode {
			Mode::Text { out_filename: None }
			| Mode::SingleFile {
				out: Output::Stdout,
			} => io::stdout().into(),

			Mode::Text {
				out_filename: Some(ref name),
			}
			| Mode::SingleFile {
				out: Output::Filename(ref name),
			} => File::create(name).map_err(as_internal_error)?.into(),

			Mode::MultipleFiles { out_dir } => {
				let base_dir = Path::new(out_dir.as_deref().unwrap_or("."));
				let file_name = base_dir.join(
					safe_path(field.file_name().ok_or(StatusCode::BAD_REQUEST)?)
						.ok_or(StatusCode::BAD_REQUEST)?,
				);
				File::create(file_name).map_err(as_internal_error)?.into()
			},

			Mode::SingleFile {
				out: Output::ClientFilename,
			} => File::create(
				safe_path(field.file_name().ok_or(StatusCode::BAD_REQUEST)?)
					.ok_or(StatusCode::BAD_REQUEST)?,
			)
			.map_err(as_internal_error)?
			.into(),
		})
	}
}

impl AsMut<dyn Write + Send> for FileOrStdout {
	fn as_mut(&mut self) -> &mut (dyn Write + Send + 'static) {
		match *self {
			FileOrStdout::File(ref mut f) => f,
			FileOrStdout::Stdout(ref mut s) => s,
		}
	}
}

impl From<File> for FileOrStdout {
	fn from(value: File) -> Self {
		Self::File(value)
	}
}

impl From<Stdout> for FileOrStdout {
	fn from(value: Stdout) -> Self {
		Self::Stdout(value)
	}
}

fn safe_path(path: &str) -> Option<&OsStr> {
	Path::new(path).file_name()
}

fn as_internal_error(e: impl Debug) -> StatusCode {
	eprintln!("internal error: {e:?}");
	StatusCode::INTERNAL_SERVER_ERROR
}

pub async fn upload(
	headers: HeaderMap,
	Extension(state): Extension<&'static State>,
	mut multipart: Multipart,
) -> response::Result<UploadSuccessTemplate> {
	let content_length: usize = headers
		.get(CONTENT_LENGTH)
		.ok_or(StatusCode::BAD_REQUEST)?
		.to_str()
		.map_err(|_| StatusCode::BAD_REQUEST)?
		.parse()
		.map_err(|_| StatusCode::BAD_REQUEST)?;

	if content_length > state.args.limit {
		// consume rest of request so we don't reset the connection
		while let Some(mut field) = multipart.next_field().await? {
			while field.chunk().await?.is_some() {}
		}
		return Err(StatusCode::PAYLOAD_TOO_LARGE.into());
	}

	let boundary_length = headers
		.get("content-type")
		.ok_or(StatusCode::BAD_REQUEST)?
		.len();
	let size_estimate = content_length
		.saturating_sub(boundary_length)
		.saturating_sub(128);

	if let Mode::MultipleFiles {
		out_dir: Some(ref dir_name),
	} = state.args.mode
	{
		DirBuilder::new()
			.recursive(true)
			.create(dir_name)
			.map_err(as_internal_error)?
	}

	let mut progress = Progress::new(size_estimate);

	while let Some(mut field) = multipart.next_field().await? {
		if field.name() != Some("data") {
			continue;
		}

		let mut out = FileOrStdout::from_mode(&state.args.mode, &field)?;
		if !state.args.quiet {
			if let Some(name) = field.file_name() {
				let _ = progress.new_file(name);
			}
		}

		while let Some(chunk) = field.chunk().await? {
			out.as_mut().write_all(&chunk).map_err(as_internal_error)?;
			if !state.args.quiet {
				let _ = progress.update(chunk.len());
			}
		}
		out.flush().map_err(as_internal_error)?;

		if !matches!(state.args.mode, Mode::MultipleFiles { out_dir: _ }) {
			break;
		}
	}

	if !state.args.keep_running {
		let sender = state.close_sender.lock().unwrap().take().unwrap();
		sender.send(()).unwrap();
	}

	Ok(UploadSuccessTemplate {
		message: match state.args.mode {
			Mode::MultipleFiles { out_dir: _ } => "Your files have been uploaded.",
			Mode::SingleFile { out: _ } => "Your file has been uploaded.",
			Mode::Text { out_filename: _ } => "Your text has been sent.",
		},
		keep_running: state.args.keep_running,
	})
}
