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
    xrefs: Option<Vec<String>>,
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
    jmp: Option<String>,
}

fn function_detail(func: &crate::function::Function) -> FunctionDetail<'_> {
    FunctionDetail {
        ip: func.ip.to_string(),
        name: func.name.as_deref(),
        desc: func.desc.as_deref(),
        xrefs: func
            .xrefs
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::function::Function;
    use runtime::SegOfs;

    #[tokio::test]
    async fn function_details_include_disassembly_and_lookup_errors() {
        use crate::function::{Block, Instr, XRef};
        let ip = SegOfs::new(0x1234, 0x5678);
        let mut decoder = iced_x86::Decoder::with_ip(
            16,
            &[0x90, 0xc3],
            ip.ofs as u64,
            iced_x86::DecoderOptions::NONE,
        );
        let func = Function {
            ip,
            name: Some("example".into()),
            blocks: vec![Block {
                ip,
                instrs: vec![
                    Instr {
                        iced: decoder.decode(),
                        comment: Some("entry".into()),
                        jmp: Some(XRef::Name("target".into())),
                    },
                    Instr {
                        iced: decoder.decode(),
                        comment: None,
                        jmp: None,
                    },
                ],
            }],
            ..Default::default()
        };
        let json = serde_json::to_value(function_detail(&func)).unwrap();
        assert_eq!(json["name"], "example");
        assert_eq!(json["blocks"][0]["ip"], "1234:5678");
        assert_eq!(
            json["blocks"][0]["instrs"],
            serde_json::json!([
                { "ip": "1234:5678", "text": "nop", "comment": "entry", "jmp": "target" },
                { "ip": "1234:5679", "text": "ret", "comment": null, "jmp": null },
            ])
        );
        let db = Arc::new(RwLock::new(DB {
            functions: [(ip, func)].into(),
            ..Default::default()
        }));
        let response = get_function(State(db.clone()), axum::extract::Path("1234:5678".into()))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response_json(response).await, json);
        assert_eq!(
            get_function(State(db.clone()), axum::extract::Path("1234:0000".into()))
                .await
                .unwrap_err(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            get_function(State(db), axum::extract::Path("invalid".into()))
                .await
                .unwrap_err(),
            StatusCode::BAD_REQUEST
        );
    }

    async fn response_json(response: Response) -> serde_json::Value {
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        serde_json::from_slice(&body).unwrap()
    }

    #[tokio::test]
    async fn handlers_read_current_db_state() {
        let db = Arc::new(RwLock::new(DB::default()));
        let initial = response_json(get_overview(State(db.clone())).await).await;
        assert_eq!(initial["mem_size"], 0);
        assert_eq!(initial["functions"], serde_json::json!([]));
        let ip = SegOfs::new(0x1234, 0x5678);
        {
            let mut db = db.write().await;
            db.mem.resize(32, 0);
            db.functions.insert(
                ip,
                Function {
                    ip,
                    name: Some("new".into()),
                    ..Default::default()
                },
            );
        }
        let updated = response_json(get_overview(State(db.clone())).await).await;
        assert_eq!(updated["mem_size"], 32);
        assert_eq!(updated["functions"][0]["name"], "new");
        let detail = get_function(State(db.clone()), axum::extract::Path(ip.to_string()))
            .await
            .unwrap();
        assert_eq!(response_json(detail).await["name"], "new");
        db.write().await.functions.get_mut(&ip).unwrap().name = Some("renamed".into());
        let detail = get_function(State(db), axum::extract::Path(ip.to_string()))
            .await
            .unwrap();
        assert_eq!(response_json(detail).await["name"], "renamed");
    }

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
