use std::path::PathBuf;

use crate::db::DB;

/// init
#[derive(argh::FromArgs)]
#[argh(subcommand, name = "init")]
pub struct Args {
    /// exe file name
    #[argh(positional)]
    exe: String,
}

pub fn init(project_path: PathBuf, args: Args) -> Result<(), Box<dyn std::error::Error>> {
    let mut db = DB::new(project_path);
    db.exe = args.exe;
    db.write()?;
    println!("wrote {}", db.meta().display());
    Ok(())
}
