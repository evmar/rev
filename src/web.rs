use axum::{Router, response::Html, routing::get};

/// serve the web interface
#[derive(argh::FromArgs)]
#[argh(subcommand, name = "web")]
pub struct Args {}

pub async fn run(_args: Args) -> anyhow::Result<()> {
    let app = Router::new().route("/", get(hello));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;
    eprintln!("Listening on http://{}", listener.local_addr()?);
    axum::serve(listener, app).await?;
    Ok(())
}

async fn hello() -> Html<&'static str> {
    Html(
        "<!doctype html><html><head><title>Hello, world!</title></head><body><h1>Hello, world!</h1></body></html>",
    )
}
