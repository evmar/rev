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
    let db = DB {
        project_path,
        exe: args.exe,
    };
    let path = db.write()?;
    println!("wrote {}", path.display());
    Ok(())
}
