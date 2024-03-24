mod args;
mod index;

use std::{net::Ipv4Addr, time::Instant};

use axum::{
	extract::{DefaultBodyLimit, Multipart},
	http::{
		header::{HeaderMap, CONTENT_LENGTH},
		StatusCode,
	},
	response,
	routing::{get, post},
	Router,
};
use tower::ServiceBuilder;
use tower_http::catch_panic::CatchPanicLayer;

use index::IndexTemplate;

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

async fn upload(headers: HeaderMap, mut multipart: Multipart) -> response::Result<()> {
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

	while let Some(mut field) = multipart.next_field().await? {
		dbg!(&field);
		println!(
			"starting field {} / {}",
			field.name().unwrap(),
			field.file_name().unwrap()
		);
		let mut total: usize = 0;
		let start = Instant::now();
		while let Some(chunk) = field.chunk().await? {
			// println!("got {} bytes", chunk.len());
			total += chunk.len();
		}
		println!("total = {total}");
		let ms = start.elapsed().as_millis();
		let rate = if ms == 0 {
			99999
		} else {
			(total as u128) / ms / 1000
		};
		println!("{rate} MB/s");
		println!("error = {}", (size_estimate as isize) - (total as isize));
	}
	Ok(())
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
	println!("{args:?}");

	let index = IndexTemplate::new(&args.mode);

	let mut app = Router::new()
		.route("/", get(move || async move { index }))
		.route("/upload", post(upload))
		.layer(ServiceBuilder::new().layer(CatchPanicLayer::new()))
		.layer(DefaultBodyLimit::max(args.limit));

	app = add_static_files(
		app,
		&[(
			"/main.css",
			"text/css",
			include_bytes!("../static/main.css"),
		)],
	);

	let listener = tokio::net::TcpListener::bind((Ipv4Addr::UNSPECIFIED, args.port)).await?;
	eprintln!(
		"listening at http://localhost:{}",
		listener.local_addr()?.port(),
	);
	axum::serve(listener, app).await
}
