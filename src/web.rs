use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

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

use crate::{db::DB, load::EXE, xref::XRef};

#[derive(serde::Serialize, ts_rs::TS)]
#[ts(export, export_to = "../web/src/bindings/")]
struct FunctionDetail<'a> {
    ip: String,
    name: Option<&'a str>,
    desc: Option<&'a str>,
    details: Option<&'a str>,
    callers: Option<Vec<XRefDetail>>,
    callees: Option<Vec<XRefDetail>>,
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
    jmp: Option<XRefDetail>,
}

/// Display name and target IP; an empty IP denotes a non-function reference.
#[derive(serde::Serialize, ts_rs::TS)]
#[ts(export_to = "../web/src/bindings/")]
struct XRefDetail(String, String);

impl From<&XRef> for XRefDetail {
    fn from(xref: &XRef) -> Self {
        match xref {
            XRef::External(name, ip) => {
                let ip = ip.to_string();
                Self(name.clone().unwrap_or_else(|| ip.clone()), ip)
            }
            XRef::Block(Some(label), _) => Self(label.clone(), String::new()),
            _ => Self(xref.to_string(), String::new()),
        }
    }
}

fn function_ref(xref: &XRef) -> Option<XRefDetail> {
    match xref {
        XRef::External(_, _) => Some(xref.into()),
        _ => None,
    }
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
            .map(|refs| refs.iter().filter_map(function_ref).collect()),
        callees: func
            .callees
            .as_ref()
            .map(|refs| refs.iter().filter_map(function_ref).collect()),
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
                        jmp: instr.jmp.as_ref().map(XRefDetail::from),
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
    let ip = ip.strip_suffix(".json").ok_or(StatusCode::BAD_REQUEST)?;
    let ip = runtime::SegOfs::parse(ip).map_err(|_| StatusCode::BAD_REQUEST)?;
    let db = db.read().await;
    let func = db.functions.get(&ip).ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(function_detail(func)).into_response())
}

async fn get_memory(State(db): State<AppState>) -> Response {
    let db = db.read().await;
    Json(memory_detail(&db)).into_response()
}

#[derive(serde::Serialize, ts_rs::TS)]
#[ts(export, export_to = "../web/src/bindings/")]
struct MemoryDetail<'a> {
    entries: Vec<MemoryEntry<'a>>,
}

#[derive(serde::Serialize, ts_rs::TS)]
#[ts(export_to = "../web/src/bindings/")]
struct MemoryEntry<'a> {
    addr: String,
    name: &'a str,
    desc: &'a str,
    typ: &'a str,
}

fn memory_detail(db: &DB) -> MemoryDetail<'_> {
    MemoryDetail {
        entries: db
            .memory
            .entries
            .iter()
            .map(|(addr, location)| MemoryEntry {
                addr: addr.to_string(),
                name: &location.name,
                desc: &location.desc,
                typ: &location.typ,
            })
            .collect(),
    }
}

#[derive(serde::Serialize, ts_rs::TS)]
#[ts(export, export_to = "../web/src/bindings/")]
struct Overview<'a> {
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
    /// export static API data to a directory instead of starting the server
    #[argh(option)]
    pub export: Option<PathBuf>,
}

fn export_site(db: &DB, output: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(output)?;
    let api = output.join("api");
    // Require fresh API data so old functions cannot survive an export.
    std::fs::create_dir(&api)?;
    std::fs::create_dir_all(api.join("functions"))?;
    std::fs::write(
        api.join("overview.json"),
        serde_json::to_vec(&overview(db))?,
    )?;
    std::fs::write(
        api.join("memory.json"),
        serde_json::to_vec(&memory_detail(db))?,
    )?;
    for func in db.functions.values() {
        std::fs::write(
            api.join("functions").join(format!("{}.json", func.ip)),
            serde_json::to_vec(&function_detail(func))?,
        )?;
    }
    Ok(())
}

pub async fn run(db: DB, args: Args) -> anyhow::Result<()> {
    let dist = Path::new(env!("CARGO_MANIFEST_DIR")).join("web/dist");
    if let Some(output) = args.export {
        anyhow::ensure!(!args.dev, "--export cannot be combined with --dev");
        export_site(&db, &output)?;
        println!("exported site to {}", output.display());
        return Ok(());
    }
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
        anyhow::ensure!(
            dist.join("index.html").is_file(),
            "Frontend build missing; run `npm --prefix web run build` first"
        );
        Router::new().fallback_service(ServeDir::new(dist))
    };
    let app = app
        .route("/api/overview.json", get(get_overview))
        .route("/api/memory.json", get(get_memory))
        .route("/api/functions/{ip}", get(get_function))
        .with_state(db);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;
    eprintln!("Listening on http://{}", listener.local_addr()?);
    axum::serve(listener, app).await?;
    Ok(())
}
