use openrouter_rs::types::TypedTool;
use runtime::SegOfs;

use crate::db::DB;

#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct GetFunctionTool {
    /// address of function to get
    addr: String,
}

impl TypedTool for GetFunctionTool {
    fn name() -> &'static str {
        "get_function"
    }

    fn description() -> &'static str {
        "get a function's name, description, and a dump of its assembly code"
    }
}

impl GetFunctionTool {
    pub fn call(db: &DB, params: &GetFunctionTool) -> anyhow::Result<String> {
        let addr = SegOfs::parse(&params.addr)
            .map_err(|_| anyhow::anyhow!("bad address {:?}", params.addr))?;
        let func = db
            .functions
            .get(&addr)
            .ok_or_else(|| anyhow::anyhow!("unknown function {addr}"))?;
        println!(
            "Reading function {} ({})",
            params.addr,
            func.name.as_deref().unwrap_or("unknown name")
        );
        let mut buf = String::new();
        func.serialize(&mut buf)?;
        Ok(buf)
    }
}
