mod args;
mod index;
mod upload;

use std::{net::Ipv4Addr, sync::Arc};

use axum::{
	extract::DefaultBodyLimit,
	routing::{get, post},
	Extension, Router,
};
use tower::ServiceBuilder;
use tower_http::catch_panic::CatchPanicLayer;

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

#[tokio::main]
async fn main() -> std::io::Result<()> {
	let args = match args::parse() {
		Ok(args) => args,
		Err(e) => {
			eprintln!("{e}");
			std::process::exit(1);
		},
	};
	let port = args.port;

	let index = IndexTemplate::new(&args.mode);

	let mut app = Router::new()
		.route("/", get(move || async move { index }))
		.route("/upload", post(upload))
		.layer(ServiceBuilder::new().layer(CatchPanicLayer::new()))
		.layer(DefaultBodyLimit::max(args.limit))
		.layer(Extension(Arc::new(args)));

	app = add_static_files(
		app,
		&[(
			"/main.css",
			"text/css",
			include_bytes!("../static/main.css"),
		)],
	);

	let listener = tokio::net::TcpListener::bind((Ipv4Addr::UNSPECIFIED, port)).await?;
	eprintln!(
		"listening at http://localhost:{}",
		listener.local_addr()?.port(),
	);
	axum::serve(listener, app).await
}
