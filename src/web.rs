use std::{collections::BTreeMap, path::Path, sync::Arc};

use axum::{
    Router,
    body::Bytes,
    http::{StatusCode, header},
    routing::get,
};
use axum_vite::ViteConfig;
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

fn lookup_function(
    functions: &BTreeMap<runtime::SegOfs, Bytes>,
    ip: &str,
) -> Result<Bytes, StatusCode> {
    let ip = runtime::SegOfs::parse(ip).map_err(|_| StatusCode::BAD_REQUEST)?;
    functions.get(&ip).cloned().ok_or(StatusCode::NOT_FOUND)
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

pub async fn run(db: &mut DB, args: Args) -> anyhow::Result<()> {
    let overview = Bytes::from(serde_json::to_vec(&overview(db))?);
    let functions = Arc::new(
        db.functions
            .iter()
            .map(|(ip, func)| {
                Ok((
                    *ip,
                    Bytes::from(serde_json::to_vec(&function_detail(func))?),
                ))
            })
            .collect::<Result<BTreeMap<_, _>, serde_json::Error>>()?,
    );
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
    let app = app.route(
        "/api/functions/{ip}",
        get(
            move |axum::extract::Path(ip): axum::extract::Path<String>| {
                let functions = functions.clone();
                async move {
                    lookup_function(&functions, &ip)
                        .map(|body| ([(header::CONTENT_TYPE, "application/json")], body))
                }
            },
        ),
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
    fn function_details_include_disassembly_and_lookup_errors() {
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
        let body = Bytes::from(serde_json::to_vec(&json).unwrap());
        let functions = BTreeMap::from([(ip, body.clone())]);
        assert_eq!(lookup_function(&functions, "1234:5678"), Ok(body));
        assert_eq!(
            lookup_function(&functions, "1234:0000"),
            Err(StatusCode::NOT_FOUND)
        );
        assert_eq!(
            lookup_function(&functions, "invalid"),
            Err(StatusCode::BAD_REQUEST)
        );
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
