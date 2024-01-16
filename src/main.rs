use askama::Template;
use axum::{extract::Path, routing::get, Router};

#[derive(Template)]
#[template(path = "hello.html")]
struct HelloTemplate<'a> {
	name: &'a str,
}

async fn hello(Path(name): Path<String>) -> String {
	HelloTemplate { name: &name }.render().unwrap()
}

#[tokio::main]
async fn main() {
	let app = Router::new()
		.route("/", get(|| async { "Hello, world!" }))
		.route("/hello/:name", get(hello));
	let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
	axum::serve(listener, app).await.unwrap();
}
