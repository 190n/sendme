mod args;
mod fmt;
mod index;
mod progress;
mod upload;

use std::{net::Ipv4Addr, sync::Mutex};

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
use tokio::sync::oneshot;
use tower_http::catch_panic::CatchPanicLayer;

use args::Args;
use fmt::Bytes;
use index::IndexTemplate;
use upload::upload;

pub struct State {
	pub args: Args,
	pub close_sender: Mutex<Option<oneshot::Sender<()>>>,
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
		close_sender: Mutex::new(if args.keep_running { None } else { Some(tx) }),
		args,
	}));

	let index = IndexTemplate::new(&state.args.mode);

	let app = Router::new()
		.route("/", get(move || async move { index }))
		.route("/upload", post(upload))
		// allow leeway here so that overflows are precisely caught by Content-Length check
		.layer(DefaultBodyLimit::disable())
		.layer(middleware::from_fn(better_errors))
		.layer(Extension(state))
		.layer(CatchPanicLayer::new());

	let listener = tokio::net::TcpListener::bind((Ipv4Addr::UNSPECIFIED, state.args.port)).await?;
	eprintln!(
		"listening at http://localhost:{}",
		listener.local_addr()?.port(),
	);

	let serve = axum::serve(listener, app);
	if state.args.keep_running {
		serve.await
	} else {
		serve
			.with_graceful_shutdown(async move { rx.await.unwrap() })
			.await
	}
}
