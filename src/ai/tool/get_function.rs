use openrouter_rs::types::TypedTool;
use runtime::SegOfs;

use crate::{api, db::DB};

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
    pub fn call(db: &mut DB, params: &GetFunctionTool) -> anyhow::Result<String> {
        let addr = SegOfs::parse(&params.addr)
            .map_err(|_| anyhow::anyhow!("bad address {:?}", params.addr))?;
        let func = db
            .functions
            .get_mut(&addr)
            .ok_or_else(|| anyhow::anyhow!("unknown function {addr}"))?;
        func.disasm(&db.mem);
        assert!(!func.blocks.is_empty());
        println!(
            "Reading function {} ({})",
            params.addr,
            func.name.as_deref().unwrap_or("unknown name")
        );
        Ok(api::plain_text(func))
    }
}
