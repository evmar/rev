use std::path::PathBuf;

use runtime::SegOfs;

use crate::{db::DB, eval};

/// load and save db
#[derive(argh::FromArgs)]
#[argh(subcommand, name = "roundtrip")]
pub struct Args {
    #[argh(positional, from_str_fn(SegOfs::parse))]
    fns: Vec<SegOfs>,
}

pub fn run(project: PathBuf, args: Args) -> anyhow::Result<()> {
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
