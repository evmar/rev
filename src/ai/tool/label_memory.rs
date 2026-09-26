#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct Response {
    /// short name for the memory location
    pub name: String,
    /// description of what the memory holds
    pub desc: String,
    /// value type as expressed in Rust, e.g. `u32` or `CStr`
    pub typ: String,
}

pub fn x() {
    db.memory
        .entries
        .insert(addr, MemoryLocation { name, desc, typ });
}
