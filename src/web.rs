use std::{path::Path, sync::Arc};

use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
};
use axum_vite::ViteConfig;
use tokio::sync::RwLock;
use tower_http::services::ServeDir;

use crate::{db::DB, load::EXE};

#[derive(serde::Serialize, ts_rs::TS)]
#[ts(export, export_to = "../web/src/bindings/")]
struct FunctionDetail<'a> {
    ip: String,
    name: Option<&'a str>,
    desc: Option<&'a str>,
    details: Option<&'a str>,
    callers: Option<Vec<String>>,
    callees: Option<Vec<String>>,
    params: &'a Option<Vec<crate::ai::Var>>,
    ret: &'a Option<crate::ai::Var>,
    blocks: Vec<BlockDetail>,
}

#[derive(serde::Serialize, ts_rs::TS)]
#[ts(export_to = "../web/src/bindings/")]
struct BlockDetail {
    ip: String,
    instrs: Vec<InstructionDetail>,
}

#[derive(serde::Serialize, ts_rs::TS)]
#[ts(export_to = "../web/src/bindings/")]
struct InstructionDetail {
    ip: String,
    text: String,
    comment: Option<String>,
    label: Option<String>,
    jmp: Option<String>,
}

fn function_detail(func: &crate::function::Function) -> FunctionDetail<'_> {
    FunctionDetail {
        ip: func.ip.to_string(),
        name: func.name.as_deref(),
        desc: func.desc.as_deref(),
        details: func.details.as_deref(),
        callers: func
            .callers
            .as_ref()
            .map(|refs| refs.iter().map(ToString::to_string).collect()),
        callees: func
            .callees
            .as_ref()
            .map(|refs| refs.iter().map(ToString::to_string).collect()),
        params: &func.params,
        ret: &func.ret,
        blocks: func
            .blocks
            .iter()
            .map(|block| BlockDetail {
                ip: block.ip.to_string(),
                instrs: block
                    .instrs
                    .iter()
                    .map(|instr| InstructionDetail {
                        ip: block.ip.with_ofs(instr.iced.ip16()).to_string(),
                        text: instr.iced.to_string(),
                        comment: instr.comment.clone(),
                        label: instr.label.clone(),
                        jmp: instr.jmp.as_ref().map(ToString::to_string),
                    })
                    .collect(),
            })
            .collect(),
    }
}

type AppState = Arc<RwLock<DB>>;

async fn get_overview(State(db): State<AppState>) -> Response {
    let db = db.read().await;
    Json(overview(&db)).into_response()
}

async fn get_function(
    State(db): State<AppState>,
    axum::extract::Path(ip): axum::extract::Path<String>,
) -> Result<Response, StatusCode> {
    let ip = runtime::SegOfs::parse(&ip).map_err(|_| StatusCode::BAD_REQUEST)?;
    let db = db.read().await;
    let func = db.functions.get(&ip).ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(function_detail(func)).into_response())
}

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
    desc: Option<&'a str>,
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
                desc: func.desc.as_deref(),
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

pub async fn run(db: DB, args: Args) -> anyhow::Result<()> {
    let db = Arc::new(RwLock::new(db));
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
    let app = app
        .route("/api/overview", get(get_overview))
        .route("/api/functions/{ip}", get(get_function))
        .with_state(db);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;
    eprintln!("Listening on http://{}", listener.local_addr()?);
    axum::serve(listener, app).await?;
    Ok(())
}
