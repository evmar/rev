use std::collections::BTreeMap;

use runtime::SegOfs;

#[derive(Default, Debug, serde::Serialize, serde::Deserialize)]
pub struct Memory {
    pub entries: BTreeMap<SegOfs, MemoryAnnotation>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct MemoryAnnotation {
    /// short name for the memory location
    pub name: String,
    /// description of what the memory holds
    pub desc: String,
    /// value type as expressed in Rust, e.g. `u32` or `CStr`
    pub typ: String,
}
