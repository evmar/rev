use std::path::PathBuf;

use crate::db::DB;

mod ai;
mod api;
mod cmd;
mod crawl;
mod db;
mod dis;
mod eval;
mod ir;
mod load;
mod web;

#[derive(argh::FromArgs)]
#[argh(subcommand)]
enum Command {
    Init(cmd::init::Args),
    Dump(cmd::dump::Args),
    Func(cmd::func::Args),
    Crawl(crawl::Args),
    RoundTrip(cmd::roundtrip::Args),
    Web(web::Args),
    Queue(cmd::queue::Args),
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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let Args { project, cmd } = argh::from_env::<Args>();
    match cmd {
        Command::Web(args) => {
            let db = DB::load(project)?;
            web::run(db, args).await?;
            Ok(())
        }
        Command::Init(args) => cmd::init::init(project, args),
        Command::Dump(args) => {
            let mut db = DB::load(project)?;
            cmd::dump::dump(&mut db, args)
        }
        Command::Func(args) => {
            let mut db = DB::load(project)?;
            cmd::func::run(&mut db, args)?;
            Ok(())
        }
        Command::RoundTrip(args) => {
            cmd::roundtrip::run(project, args)?;
            Ok(())
        }
        Command::Crawl(args) => {
            let mut db = DB::load(project)?;
            crawl::run(&mut db, args)?;
            Ok(())
        }
        Command::Queue(args) => {
            cmd::queue::run(project, args).await?;
            Ok(())
        }
    }
}
