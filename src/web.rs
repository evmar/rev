use std::{
    collections::HashMap,
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
use runtime::SegOfs;
use tokio::sync::RwLock;
use tower_http::services::ServeDir;

use crate::{
    db::{DB, Memory, XRef},
    load::EXE,
};

#[derive(serde::Serialize, ts_rs::TS)]
#[ts(export)]
struct FunctionDetail<'a> {
    ip: String,
    name: Option<&'a str>,
    desc: Option<&'a str>,
    details: Option<&'a str>,
    callers: Option<Vec<XRefDetail>>,
    callees: Option<Vec<XRefDetail>>,
    params: &'a Vec<crate::api::Var>,
    ret: &'a Option<crate::api::Var>,
    annotations: HashMap<SegOfs, AnnotationDetail<'a>>,
    #[ts(type = "Record<string, string>")]
    memory: HashMap<SegOfs, &'a str>,
    blocks: Vec<BlockDetail>,
}

#[derive(serde::Serialize, ts_rs::TS)]
#[ts(export)]
struct BlockDetail {
    ip: SegOfs,
    instrs: Vec<InstructionDetail>,
}

#[derive(serde::Serialize, ts_rs::TS)]
#[ts(export)]
struct InstructionDetail {
    ip: SegOfs,
    text: String,
}

#[derive(serde::Serialize, ts_rs::TS)]
#[ts(export)]
struct AnnotationDetail<'a> {
    comment: Option<&'a str>,
    label: Option<&'a str>,
    jmp: Option<XRefDetail>,
    memory: Option<MemoryDetail>,
    assume: &'a Vec<(String, u16)>,
}

#[derive(serde::Serialize, ts_rs::TS)]
#[ts(export)]
enum MemoryDetail {
    Address(String),
    Note(String),
}

/// Display name and target IP; an empty IP denotes a non-function reference.
#[derive(serde::Serialize, ts_rs::TS)]
#[ts(export)]
struct XRefDetail(String, SegOfs);

impl From<&XRef> for XRefDetail {
    fn from(xref: &XRef) -> Self {
        match xref {
            XRef::External(name, ip) => Self(name.clone().unwrap_or_else(|| ip.to_string()), *ip),
            XRef::Block(Some(label), _) => Self(label.clone(), SegOfs::default()),
            _ => Self(xref.to_string(), SegOfs::default()),
        }
    }
}

fn function_ref(xref: &XRef) -> Option<XRefDetail> {
    match xref {
        XRef::External(_, _) => Some(xref.into()),
        _ => None,
    }
}

fn function_detail<'a>(memory: &'a Memory, func: &'a crate::db::Function) -> FunctionDetail<'a> {
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
        memory: func
            .annotations
            .values()
            .filter_map(|annotation| annotation.memory.as_ref()?.as_ref().ok())
            .filter_map(|addr| {
                memory
                    .entries
                    .get(addr)
                    .map(|location| (*addr, location.name.as_str()))
            })
            .collect(),
        annotations: func
            .annotations
            .iter()
            .map(|(ip, annotation)| {
                (
                    *ip,
                    AnnotationDetail {
                        comment: annotation.comment.as_deref(),
                        label: annotation.label.as_deref(),
                        jmp: annotation.jmp.as_ref().map(Into::into),
                        memory: annotation.memory.as_ref().map(|memory| match memory {
                            Ok(addr) => MemoryDetail::Address(addr.to_string()),
                            Err(note) => MemoryDetail::Note(note.clone()),
                        }),
                        assume: &annotation.assume,
                    },
                )
            })
            .collect(),
        blocks: func
            .blocks
            .iter()
            .map(|block| BlockDetail {
                ip: block.ip,
                instrs: block
                    .instrs
                    .iter()
                    .map(|instr| InstructionDetail {
                        ip: block.ip.with_ofs(instr.iced.ip16()),
                        text: instr.iced.to_string(),
                    })
                    .collect(),
            })
            .collect(),
    }
}

type AppState = Arc<RwLock<DB>>;

const MEMORY_PAGE_SIZE: usize = 4096;

fn memory_bytes(db: &DB, addr: SegOfs) -> &[u8] {
    let start = (addr.abs() as usize).min(db.mem.len());
    &db.mem[start..(start + MEMORY_PAGE_SIZE).min(db.mem.len())]
}

async fn get_memory_bytes(
    State(db): State<AppState>,
    axum::extract::Path(addr): axum::extract::Path<String>,
) -> Result<Response, StatusCode> {
    let addr = addr.strip_suffix(".json").ok_or(StatusCode::BAD_REQUEST)?;
    let addr = SegOfs::parse(addr).map_err(|_| StatusCode::BAD_REQUEST)?;
    let db = db.read().await;
    let bytes = memory_bytes(&db, addr);
    Ok(Json(bytes).into_response())
}

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
    let db = &mut *db.write().await;
    let func = db.functions.get_mut(&ip).ok_or(StatusCode::NOT_FOUND)?;
    func.disasm(&db.mem);
    Ok(Json(function_detail(&db.memory, func)).into_response())
}

async fn get_memory(State(db): State<AppState>) -> Response {
    let db = db.read().await;
    Json(memory_entries(&db)).into_response()
}

async fn get_memory_address(
    State(db): State<AppState>,
    axum::extract::Path(addr): axum::extract::Path<String>,
) -> Result<Response, StatusCode> {
    let addr = addr.strip_suffix(".json").ok_or(StatusCode::BAD_REQUEST)?;
    let addr = runtime::SegOfs::parse(addr).map_err(|_| StatusCode::BAD_REQUEST)?;
    let db = db.read().await;
    let detail = memory_address_detail(&db, addr).ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(detail).into_response())
}

#[derive(serde::Serialize, ts_rs::TS)]
#[ts(export)]
struct MemoryEntries<'a> {
    entries: Vec<MemoryEntry<'a>>,
}

#[derive(serde::Serialize, ts_rs::TS)]
#[ts(export)]
struct MemoryEntry<'a> {
    addr: String,
    name: &'a str,
    desc: &'a str,
    typ: &'a str,
}

fn memory_entries(db: &DB) -> MemoryEntries<'_> {
    MemoryEntries {
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
#[ts(export)]
struct MemoryAddressDetail<'a> {
    addr: String,
    name: &'a str,
    desc: &'a str,
    typ: &'a str,
    functions: Vec<FunctionOverview<'a>>,
}

fn memory_address_detail(db: &DB, addr: runtime::SegOfs) -> Option<MemoryAddressDetail<'_>> {
    let location = db.memory.entries.get(&addr)?;
    let functions = db
        .functions
        .values()
        .filter(|func| {
            func.annotations.values().any(|annot| {
                annot
                    .memory
                    .as_ref()
                    .and_then(|memory| memory.as_ref().ok())
                    .is_some_and(|memory| *memory == addr)
            })
        })
        .map(|func| FunctionOverview {
            ip: func.ip,
            name: func.name.as_deref(),
            desc: func.desc.as_deref(),
        })
        .collect();
    Some(MemoryAddressDetail {
        addr: addr.to_string(),
        name: &location.name,
        desc: &location.desc,
        typ: &location.typ,
        functions,
    })
}

#[derive(serde::Serialize, ts_rs::TS)]
#[ts(export)]
struct Overview<'a> {
    mem_size: usize,
    exe: &'a EXE,
    functions: Vec<FunctionOverview<'a>>,
}

#[derive(serde::Serialize, ts_rs::TS)]
#[ts(export)]
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
    std::fs::create_dir_all(api.join("memory"))?;
    std::fs::write(
        api.join("overview.json"),
        serde_json::to_vec(&overview(db))?,
    )?;
    std::fs::write(
        api.join("memory.json"),
        serde_json::to_vec(&memory_entries(db))?,
    )?;
    for func in db.functions.values() {
        std::fs::write(
            api.join("functions").join(format!("{}.json", func.ip)),
            serde_json::to_vec(&function_detail(&db.memory, func))?,
        )?;
    }
    for addr in db.memory.entries.keys() {
        let detail = memory_address_detail(db, *addr).expect("memory address must exist");
        std::fs::write(
            api.join("memory").join(format!("{addr}.json")),
            serde_json::to_vec(&detail)?,
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
        .route("/api/memory/{addr}", get(get_memory_address))
        .route("/api/bytes/{addr}", get(get_memory_bytes))
        .route("/api/functions/{ip}", get(get_function))
        .with_state(db);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;
    eprintln!("Listening on http://{}", listener.local_addr()?);
    axum::serve(listener, app).await?;
    Ok(())
}
