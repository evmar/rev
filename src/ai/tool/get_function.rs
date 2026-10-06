use openrouter_rs::types::TypedTool;
use runtime::SegOfs;

use crate::{api, db::DB};

#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct GetFunctionTool {
    /// address of function to get
    addr: String,

    /// when false, return only name/arguments/description.
    /// when true, include the function's body assembly.
    include_body: bool,
}

impl TypedTool for GetFunctionTool {
    fn name() -> &'static str {
        "get_function"
    }

    fn description() -> &'static str {
        "get a function's name, description, and optionally a dump of its assembly code"
    }
}

impl GetFunctionTool {
    pub fn call(
        db: &DB,
        GetFunctionTool { addr, include_body }: GetFunctionTool,
    ) -> anyhow::Result<String> {
        println!("get_func({addr}, include_body={include_body})");
        let addr = SegOfs::parse(&addr).map_err(|_| anyhow::anyhow!("bad address {:?}", addr))?;
        let func = db
            .functions
            .get(&addr)
            .ok_or_else(|| anyhow::anyhow!("unknown function {addr}"))?;
        assert!(!func.blocks.is_empty());
        println!("-> success");
        Ok(api::plain_text(func, include_body))
    }
}
