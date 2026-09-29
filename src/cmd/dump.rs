use std::path::PathBuf;

use crate::{api, db::DB};

/// dump program database as text
#[derive(argh::FromArgs)]
#[argh(subcommand, name = "dump")]
pub struct Args {
    /// directory to dump to
    #[argh(positional)]
    out_dir: PathBuf,
}

pub fn dump(db: &mut DB, Args { out_dir }: Args) -> anyhow::Result<()> {
    std::fs::create_dir_all(&out_dir)?;

    std::fs::write(
        out_dir.join("db.toml"),
        toml::to_string(&db.exe)?.as_bytes(),
    )?;
    std::fs::write(
        out_dir.join("memory.toml"),
        toml::to_string(&db.memory)?.as_bytes(),
    )?;

    let fn_dir = out_dir.join("fn");
    std::fs::create_dir_all(&fn_dir)?;
    for func in db.functions.values_mut() {
        func.disasm(&db.mem);
        let name = match &func.name {
            Some(name) => format!("{name}.toml"),
            None => format!("{:04x}_{:04x}.toml", func.ip.seg, func.ip.ofs),
        };
        let path = fn_dir.join(name);
        std::fs::write(&path, api::plain_text(func).as_bytes())?;
    }

    Ok(())
}
