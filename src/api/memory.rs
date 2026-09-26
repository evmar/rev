use runtime::SegOfs;

use crate::db::DB;

pub fn read_memory<'db>(db: &'db DB, addr: SegOfs, len: usize) -> anyhow::Result<&'db [u8]> {
    let ofs = addr.abs() as usize;
    let Some(buf) = db.mem.get(ofs..ofs + len) else {
        anyhow::bail!("invalid location");
    };
    Ok(buf)
}
