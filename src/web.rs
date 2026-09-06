use std::path::Path;

use axum::Router;
use axum_vite::ViteConfig;
use tower_http::services::ServeDir;

/// serve the web interface
#[derive(argh::FromArgs)]
#[argh(subcommand, name = "web")]
pub struct Args {
    /// proxy to the Vite development server on port 5173
    #[argh(switch)]
    pub dev: bool,
}

pub async fn run(args: Args) -> anyhow::Result<()> {
    let app = if args.dev {
        anyhow::ensure!(
            cfg!(debug_assertions),
            "axum-vite development mode requires a debug build; omit --release"
        );
        eprintln!("Proxying to Vite at http://127.0.0.1:5173; run `npm --prefix web run dev`");
        let config = ViteConfig {
            dev_host: "127.0.0.1".into(),
            prefix: "/".into(),
            ..Default::default()
        };
        Router::new().fallback_service(axum_vite::spa_router(config))
    } else {
        let dist = Path::new(env!("CARGO_MANIFEST_DIR")).join("web/dist");
        anyhow::ensure!(
            dist.join("index.html").is_file(),
            "Frontend build missing; run `npm --prefix web run build` first"
        );
        Router::new().fallback_service(ServeDir::new(dist))
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;
    eprintln!("Listening on http://{}", listener.local_addr()?);
    axum::serve(listener, app).await?;
    Ok(())
}
