mod args;
mod fmt;
mod index;
mod progress;
mod upload;

use std::{net::Ipv4Addr, process::Stdio, sync::Mutex};

use askama::Template;
use askama_axum::IntoResponse;
use axum::{
	extract::{DefaultBodyLimit, Request},
	http::StatusCode,
	middleware::{self, Next},
	routing::{get, post},
	Extension, Router,
};
use axum_core::response::Response;
use tokio::{
	io::{AsyncBufReadExt, BufReader},
	process::Command,
	sync::oneshot,
};
use tower_http::catch_panic::CatchPanicLayer;

use args::Args;
use fmt::Bytes;
use index::IndexTemplate;
use upload::upload;

pub struct State {
	pub args: Args,
	pub close_sender: Mutex<Option<oneshot::Sender<()>>>,
	pub exit_code: Mutex<i32>,
}

#[derive(Template)]
#[template(path = "error.html")]
struct ErrorTemplate {
	code: u16,
	reason: &'static str,
	explanation: Option<String>,
}

async fn better_errors(
	Extension(state): Extension<&'static State>,
	request: Request,
	next: Next,
) -> Response {
	let response = next.run(request).await.into_response();
	if !response.status().is_success() {
		// close server if we got 500
		if response.status().is_server_error() {
			if let Some(tx) = state.close_sender.lock().unwrap().take() {
				tx.send(()).unwrap();
			}
		}
		return ErrorTemplate {
			code: response.status().as_u16(),
			reason: response.status().canonical_reason().unwrap_or_default(),
			explanation: match response.status() {
				StatusCode::PAYLOAD_TOO_LARGE => Some(format!(
					"The recipient set the upload limit to {}",
					Bytes(state.args.limit),
				)),
				_ => None,
			},
		}
		.into_response();
	}
	response
}

/// paths: slice of (filename, content-type, contents)
fn add_static_files(
	mut app: Router,
	paths: &[(&'static str, &'static str, &'static [u8])],
) -> Router {
	for &(path, content_type, data) in paths {
		let response = ([("content-type", content_type)], data);
		app = app.route(path, get(response));
	}
	app
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
	let args = match args::parse() {
		Ok(args) => args,
		Err(e) => {
			eprintln!("{e}");
			std::process::exit(1);
		},
	};

	let (tx, rx) = oneshot::channel::<()>();

	let state: &'static State = Box::leak(Box::new(State {
		close_sender: Mutex::new(Some(tx)),
		args,
		exit_code: Mutex::new(0),
	}));

	let index = IndexTemplate::new(&state.args.mode, state.args.limit);

	let mut app = Router::new()
		.route("/", get(move || async move { index }))
		.route("/upload", post(upload))
		// allow leeway here so that overflows are precisely caught by Content-Length check
		.layer(DefaultBodyLimit::disable())
		.layer(middleware::from_fn(better_errors))
		.layer(Extension(state))
		.layer(CatchPanicLayer::new());

	app = add_static_files(
		app,
		&[(
			"/upload.js",
			"application/javascript",
			include_bytes!("../static/upload.js"),
		)],
	);

	let listener = tokio::net::TcpListener::bind((Ipv4Addr::UNSPECIFIED, state.args.port)).await?;
	let port = listener.local_addr()?.port();
	eprintln!("listening at http://localhost:{port}",);

	if state.args.use_tailscale_funnel {
		tokio::spawn(async move {
			let shutdown = move || {
				*state.exit_code.lock().unwrap() = 1;
				let sender = state.close_sender.lock().unwrap().take().unwrap();
				sender.send(()).unwrap();
			};

			let mut cmd = match Command::new("tailscale")
				.args(["funnel", &port.to_string()])
				.stdout(Stdio::piped())
				.stderr(Stdio::inherit())
				.spawn()
			{
				Ok(cmd) => cmd,
				Err(e) => {
					eprintln!("failed to start tailscale: {e}");
					return shutdown();
				},
			};
			let mut lines = BufReader::new(cmd.stdout.take().unwrap()).lines();

			let (Ok(Some(line1)), Ok(Some(line2)), Ok(Some(line3))) = (
				lines.next_line().await,
				lines.next_line().await,
				lines.next_line().await,
			) else {
				eprintln!("unexpected output from tailscale binary");
				return shutdown();
			};

			if line1 != "Available on the internet:" || !line2.is_empty() {
				eprint!("unexpected output from tailscale binary:\n> {line1}\n> {line2}\n");
				return shutdown();
			}
			eprintln!("funnelled at {line3}");
		});
	}

	let serve = axum::serve(listener, app);
	if state.args.keep_running {
		serve.await?;
	} else {
		serve
			.with_graceful_shutdown(async move { rx.await.unwrap() })
			.await?;
	}

	std::process::exit(*state.exit_code.lock().unwrap());
}
