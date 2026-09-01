use runtime::SegOfs;

use crate::db::DB;

#[derive(Default, serde::Serialize, serde::Deserialize)]
pub struct EXE {
    pub filename: String,
    #[serde(skip_serializing_if = "SegOfs::is_null")]
    pub entry_point: SegOfs,
}

pub fn load_exe(db: &mut DB) -> Vec<u8> {
    const DOSBOX_SEG: u16 = 0x813;
    let mut mem = Vec::<u8>::new();
    let psp_segment = DOSBOX_SEG;
    let load_addr = SegOfs::new(psp_segment + 0x10, 0);

    let path = db.project_path(&db.exe.filename);
    println!("loading {}", path.display());
    let buf = std::fs::read(path).unwrap();
    let dos = exe::DOS::parse(&buf).unwrap();
    {
        let data = &buf[dos.image_offset()..];
        mem.resize(load_addr.abs() as usize + data.len(), 0);
        mem[load_addr.abs() as usize..].copy_from_slice(data);
    }
    dos.apply_relocations(load_addr.seg, &mut mem[load_addr.abs() as usize..]);

    let cs = load_addr.seg + dos.header.initial_cs;
    db.exe.entry_point = (cs, dos.header.entry_point).into();
    mem
}
