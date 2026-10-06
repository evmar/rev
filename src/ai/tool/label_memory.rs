use runtime::SegOfs;

use crate::db::{DB, MemoryAnnotation};

#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct LabelMemoryTool {
    /// address of the memory
    pub addr: String,
    /// short name for the memory location
    pub name: String,
    /// description of what the memory holds
    pub desc: String,
    /// value type as expressed in Rust, e.g. `u32` or `CStr`
    pub typ: String,
}

impl openrouter_rs::types::TypedTool for LabelMemoryTool {
    fn name() -> &'static str {
        "label_memory"
    }

    fn description() -> &'static str {
        "write a memory location's name, description, and type"
    }
}

impl LabelMemoryTool {
    pub fn call(
        db: &mut DB,
        LabelMemoryTool {
            addr,
            name,
            desc,
            typ,
        }: LabelMemoryTool,
    ) -> anyhow::Result<String> {
        println!("label_memory({addr}, {name}, {desc}, {typ})");
        let addr = SegOfs::parse(&addr).map_err(|_| anyhow::anyhow!("bad address {:?}", addr))?;
        db.memory
            .entries
            .insert(addr, MemoryAnnotation { name, desc, typ });
        Ok("ok".into())
    }
}
