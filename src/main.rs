use std::path::PathBuf;

use runtime::SegOfs;

use crate::{
    cmd::{func, queue},
    db::DB,
};

mod ai;
mod api;
mod cmd;
mod crawl;
mod db;
mod dis;
mod eval;
mod init;
mod ir;
mod load;
mod web;
mod xref;

#[derive(argh::FromArgs)]
#[argh(subcommand)]
enum Command {
    Init(init::Args),
    Func(func::Args),
    Crawl(crawl::Args),
    RoundTrip(RoundTrip),
    Web(web::Args),
    Queue(queue::Args),
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
struct RoundTrip {
    #[argh(positional, from_str_fn(SegOfs::parse))]
    fns: Vec<SegOfs>,
}

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
        Command::Func(args) => {
            let mut db = DB::load(project)?;
            func::run(&mut db, args)?;
            Ok(())
        }
        Command::RoundTrip(args) => {
            let now = std::time::Instant::now();
            let mut db = DB::load(project)?;
            println!("load time: {:?}", now.elapsed());

            let now = std::time::Instant::now();
            eval::eval_all(&mut db, args.fns);
            println!("process time: {:?}", now.elapsed());

            let now = std::time::Instant::now();
            db.write()?;
            println!("write time: {:?}", now.elapsed());

            Ok(())
        }
        Command::Crawl(args) => {
            let mut db = DB::load(project)?;
            crawl::run(&mut db, args)?;
            Ok(())
        }
        Command::Queue(args) => {
            queue::run(project, args).await?;
            Ok(())
        }
    }
}
