use anyhow::anyhow;
use openrouter_rs::types::TypedTool;
use runtime::SegOfs;

use crate::{api, db::DB};

#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct WriteFunctionTool {
    /// address of function to write
    addr: String,

    /// updated function info
    update: api::FunctionUpdate,
}

impl TypedTool for WriteFunctionTool {
    fn name() -> &'static str {
        "write_function"
    }

    fn description() -> &'static str {
        "update information about a function"
    }
}

impl WriteFunctionTool {
    pub fn call(
        db: &mut DB,
        WriteFunctionTool { addr, update }: WriteFunctionTool,
    ) -> anyhow::Result<String> {
        let addr = SegOfs::parse(&addr).map_err(|_| anyhow!("bad address {:?}", addr))?;
        let func = db
            .functions
            .get_mut(&addr)
            .ok_or_else(|| anyhow!("no function {}", addr))?;
        update.merge_into(func)?;
        println!("write_function(): wrote {addr}");
        Ok(format!("wrote {addr}"))
    }
}
