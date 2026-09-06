use std::path::Path;

use axum::Router;
use tower_http::services::ServeDir;

/// serve the web interface
#[derive(argh::FromArgs)]
#[argh(subcommand, name = "web")]
pub struct Args {}

pub async fn run(_args: Args) -> anyhow::Result<()> {
    let dist = Path::new(env!("CARGO_MANIFEST_DIR")).join("web/dist");
    anyhow::ensure!(
        dist.join("index.html").is_file(),
        "Frontend build missing; run `npm --prefix web run build` first"
    );
    let app = Router::new().fallback_service(ServeDir::new(dist));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;
    eprintln!("Listening on http://{}", listener.local_addr()?);
    axum::serve(listener, app).await?;
    Ok(())
}
