use std::path::PathBuf;

use crate::{db::DB, load::load_exe};

/// init
#[derive(argh::FromArgs)]
#[argh(subcommand, name = "init")]
pub struct Args {
    /// exe file name
    #[argh(positional)]
    exe: String,
}

pub fn init(project_path: PathBuf, args: Args) -> Result<(), Box<dyn std::error::Error>> {
    let mut db = DB::default();
    db.project_path = project_path;
    db.exe.filename = args.exe;
    load_exe(&mut db);

    println!("entry point {}", db.exe.entry_point);

    db.write()?;
    println!("wrote {}", db.meta().display());
    Ok(())
}
