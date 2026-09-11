use std::path::PathBuf;

use crate::db::DB;

mod ai;
mod crawl;
mod db;
mod dis;
mod eval;
mod function;
mod init;
mod ir;
mod load;
mod web;
mod xref;

#[derive(argh::FromArgs)]
#[argh(subcommand)]
enum Command {
    Init(init::Args),
    Dis(dis::Args),
    AI(ai::Args),
    Crawl(crawl::Args),
    RoundTrip(RoundTrip),
    Web(web::Args),
}

/// wip
#[derive(argh::FromArgs)]
struct Args {
    /// project path
    #[argh(option)]
    project: PathBuf,

    #[argh(subcommand)]
    cmd: Command,
}

/// load and save db
#[derive(argh::FromArgs)]
#[argh(subcommand, name = "roundtrip")]
struct RoundTrip {}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let Args { project, cmd } = argh::from_env::<Args>();
    match cmd {
        Command::Web(args) => {
            let db = DB::load(project)?;
            web::run(db, args).await?;
            Ok(())
        }
        Command::Init(args) => init::init(project, args),
        Command::Dis(args) => {
            let mut db = DB::load(project)?;
            dis::run(&mut db, args)?;
            Ok(())
        }
        Command::AI(args) => {
            let mut db = DB::load(project)?;
            ai::run(&mut db, args).await?;
            Ok(())
        }
        Command::RoundTrip(_) => {
            let mut db = DB::load(project)?;
            eval::eval_all(&mut db);
            db.write()?;
            Ok(())
        }
        Command::Crawl(args) => {
            let mut db = DB::load(project)?;
            crawl::run(&mut db, args).await?;
            Ok(())
        }
    }
}
