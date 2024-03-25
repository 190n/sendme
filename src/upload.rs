use axum::{
	extract::Multipart,
	http::{
		header::{HeaderMap, CONTENT_LENGTH},
		StatusCode,
	},
	response, Extension,
};
use std::{
	ffi::OsStr,
	fs::{DirBuilder, File},
	io::{self, Stdout, Write},
	path::Path,
	sync::Arc,
	time::Instant,
};

use crate::args::{Args, Mode, Output};

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
}

impl AsMut<dyn Write + Send> for FileOrStdout {
	fn as_mut(&mut self) -> &mut (dyn Write + Send + 'static) {
		match self {
			&mut FileOrStdout::File(ref mut f) => f,
			&mut FileOrStdout::Stdout(ref mut s) => s,
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

fn as_internal_error(e: impl std::fmt::Debug) -> StatusCode {
	eprintln!("internal error: {e:?}");
	StatusCode::INTERNAL_SERVER_ERROR
}

pub async fn upload(
	headers: HeaderMap,
	Extension(args): Extension<Arc<Args>>,
	mut multipart: Multipart,
) -> response::Result<()> {
	let content_length: usize = headers
		.get(CONTENT_LENGTH)
		.ok_or(StatusCode::BAD_REQUEST)?
		.to_str()
		.map_err(|_| StatusCode::BAD_REQUEST)?
		.parse()
		.map_err(|_| StatusCode::BAD_REQUEST)?;
	let boundary_length = headers
		.get("content-type")
		.ok_or(StatusCode::BAD_REQUEST)?
		.len();
	let size_estimate = (content_length - boundary_length).saturating_sub(128);

	if let Mode::MultipleFiles {
		out_dir: Some(ref dir_name),
	} = args.mode
	{
		DirBuilder::new()
			.recursive(true)
			.create(dir_name)
			.map_err(as_internal_error)?
	}

	while let Some(mut field) = multipart.next_field().await? {
		if field.name() != Some("data") {
			continue;
		}

		let mut out: FileOrStdout = match &args.mode {
			Mode::Text { out_filename: None }
			| Mode::SingleFile {
				out: Output::Stdout,
			} => std::io::stdout().into(),

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
		};

		let mut total: usize = 0;
		let start = Instant::now();

		while let Some(chunk) = field.chunk().await? {
			out.as_mut().write_all(&chunk).map_err(as_internal_error)?;
			total += chunk.len();
		}
		out.flush().map_err(as_internal_error)?;

		eprintln!("total = {total}");
		let ms = start.elapsed().as_millis();
		let rate = if ms == 0 {
			99999
		} else {
			(total as u128) / ms / 1000
		};
		eprintln!("{rate} MB/s");
		eprintln!("error = {}", (size_estimate as isize) - (total as isize));

		if !matches!(args.mode, Mode::MultipleFiles { out_dir: _ }) {
			return Ok(());
		}
	}
	Ok(())
}
