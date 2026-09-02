use std::path::PathBuf;

use crate::db::DB;

mod ai;
mod db;
mod dis;
mod function;
mod init;
mod load;

#[derive(argh::FromArgs)]
#[argh(subcommand)]
enum Mode {
    Init(init::Args),
    Dis(dis::Args),
    AI(ai::Args),
    RoundTrip(RoundTrip),
}

/// wip
#[derive(argh::FromArgs)]
struct Args {
    /// project path
    #[argh(option)]
    project: PathBuf,

    #[argh(subcommand)]
    mode: Mode,
}

/// wip
#[derive(argh::FromArgs)]
#[argh(subcommand, name = "roundtrip")]
struct RoundTrip {}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let Args { project, mode } = argh::from_env::<Args>();
    match mode {
        Mode::Init(args) => init::init(project, args),
        Mode::Dis(args) => {
            let mut db = DB::load(project)?;
            dis::run(&mut db, args)?;
            Ok(())
        }
        Mode::AI(args) => {
            let mut db = DB::load(project)?;
            ai::run(&mut db, args).await?;
            Ok(())
        }
        Mode::RoundTrip(_) => {
            let db = DB::load(project)?;
            db.write()?;
            Ok(())
        }
    }
}
