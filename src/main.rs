mod args;
mod index;
mod upload;

use std::{net::Ipv4Addr, sync::Mutex};

use axum::{
	extract::DefaultBodyLimit,
	routing::{get, post},
	Extension, Router,
};
use tokio::sync::oneshot;
use tower::ServiceBuilder;
use tower_http::catch_panic::CatchPanicLayer;

use args::Args;
use index::IndexTemplate;
use upload::upload;

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

pub struct State {
	pub args: Args,
	pub close_sender: Mutex<Option<oneshot::Sender<()>>>,
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
	println!("{args:?}");

	let state: &'static State = Box::leak(Box::new(State {
		close_sender: Mutex::new(if args.keep_running { None } else { Some(tx) }),
		args,
	}));

	let index = IndexTemplate::new(&state.args.mode);

	let mut app = Router::new()
		.route("/", get(move || async move { index }))
		.route("/upload", post(upload))
		.layer(ServiceBuilder::new().layer(CatchPanicLayer::new()))
		.layer(DefaultBodyLimit::max(state.args.limit))
		.layer(Extension(state));

	app = add_static_files(
		app,
		&[(
			"/main.css",
			"text/css",
			include_bytes!("../static/main.css"),
		)],
	);

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
