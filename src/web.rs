use std::path::Path;

use axum::{Router, body::Bytes, http::header, routing::get};
use axum_vite::ViteConfig;
use tower_http::services::ServeDir;

use crate::{db::DB, load::EXE};

#[derive(serde::Serialize, ts_rs::TS)]
#[ts(export, export_to = "../web/src/bindings/")]
struct Overview<'a> {
    #[ts(type = "string")]
    project_path: &'a Path,
    mem_size: usize,
    exe: &'a EXE,
    functions: Vec<FunctionOverview<'a>>,
}

#[derive(serde::Serialize, ts_rs::TS)]
#[ts(export_to = "../web/src/bindings/")]
struct FunctionOverview<'a> {
    #[ts(type = "string")]
    ip: runtime::SegOfs,
    name: Option<&'a str>,
}

fn overview(db: &DB) -> Overview<'_> {
    Overview {
        project_path: &db.project_path,
        mem_size: db.mem.len(),
        exe: &db.exe,
        functions: db
            .functions
            .values()
            .map(|func| FunctionOverview {
                ip: func.ip,
                name: func.name.as_deref(),
            })
            .collect(),
    }
}

/// serve the web interface
#[derive(argh::FromArgs)]
#[argh(subcommand, name = "web")]
pub struct Args {
    /// proxy to the Vite development server on port 5173
    #[argh(switch)]
    pub dev: bool,
}

pub async fn run(db: &mut DB, args: Args) -> anyhow::Result<()> {
    let overview = Bytes::from(serde_json::to_vec(&overview(db))?);
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
    let app = app.route(
        "/api/overview",
        get(move || {
            let overview = overview.clone();
            async move { ([(header::CONTENT_TYPE, "application/json")], overview) }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;
    eprintln!("Listening on http://{}", listener.local_addr()?);
    axum::serve(listener, app).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::function::Function;
    use runtime::SegOfs;

    #[test]
    fn overview_serializes_metadata_and_function_summaries() {
        let ip = SegOfs::new(0x1234, 0x5678);
        let unnamed_ip = SegOfs::new(0x1234, 0x6000);
        let db = DB {
            project_path: "example-project".into(),
            mem: vec![0; 32],
            exe: EXE {
                filename: "example.exe".into(),
                entry_point: ip,
            },
            functions: [
                (
                    ip,
                    Function {
                        ip,
                        name: Some("main".into()),
                        ..Default::default()
                    },
                ),
                (
                    unnamed_ip,
                    Function {
                        ip: unnamed_ip,
                        ..Default::default()
                    },
                ),
            ]
            .into(),
        };
        assert_eq!(
            serde_json::to_value(overview(&db)).unwrap(),
            serde_json::json!({
                "project_path": "example-project",
                "mem_size": 32,
                "exe": { "filename": "example.exe", "entry_point": "1234:5678" },
                "functions": [
                    { "ip": "1234:5678", "name": "main" },
                    { "ip": "1234:6000", "name": null },
                ],
            })
        );
    }
}
