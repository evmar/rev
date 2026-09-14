use openrouter_rs::types::TypedTool;
use runtime::SegOfs;

use crate::db::DB;

#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct ReadMemoryParams {
    /// address to read
    addr: String,
    /// number of bytes to read
    len: usize,
}

impl TypedTool for ReadMemoryParams {
    fn name() -> &'static str {
        "read_memory"
    }

    fn description() -> &'static str {
        "read the program's initial memory at a given offset, returns an error message or hex bytes"
    }
}

impl ReadMemoryParams {
    pub fn call(
        db: &DB,
        ReadMemoryParams { addr, len }: ReadMemoryParams,
    ) -> anyhow::Result<String> {
        let addr = SegOfs::parse(&addr).map_err(|_| anyhow::anyhow!("bad address {:?}", addr))?;
        println!("Reading {} bytes from memory at {}", len, addr);
        let ofs = addr.abs() as usize;
        let Some(buf) = db.mem.get(ofs..ofs + len) else {
            anyhow::bail!("invalid location");
        };

        use std::fmt::Write;
        let mut str = String::new();
        for b in buf {
            write!(&mut str, " {b:02x}").ok();
        }
        Ok(str)
    }
}
