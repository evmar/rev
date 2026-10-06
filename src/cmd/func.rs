use anyhow::anyhow;
use runtime::SegOfs;

use crate::{
    api,
    db::{DB, xref},
    dis::dis_func,
};

/// functions: reading, writing
#[derive(argh::FromArgs)]
#[argh(subcommand, name = "func")]
pub struct Args {
    #[argh(subcommand)]
    cmd: Command,
}

#[derive(argh::FromArgs)]
#[argh(subcommand)]
enum Command {
    New(NewArgs),
    Read(ReadArgs),
    Write(WriteArgs),
}

pub fn run(db: &mut DB, Args { cmd }: Args) -> anyhow::Result<()> {
    match cmd {
        Command::New(args) => cmd_new(db, args),
        Command::Read(args) => cmd_read(db, args),
        Command::Write(args) => cmd_write(db, args),
    }
}

/// given an address, disassemble its code and create a new function entry for it
#[derive(argh::FromArgs)]
#[argh(subcommand, name = "new")]
struct NewArgs {
    #[argh(positional, from_str_fn(SegOfs::parse))]
    addr: SegOfs,
}

fn cmd_new(db: &mut DB, NewArgs { addr }: NewArgs) -> anyhow::Result<()> {
    dis_func(db, addr);
    let func = db.functions.get_mut(&addr).unwrap();
    db.write()?;
    Ok(())
}

/// read function: get description, arguments, and disassembly
#[derive(argh::FromArgs)]
#[argh(subcommand, name = "read")]
struct ReadArgs {
    /// function address or name
    #[argh(positional)]
    addr_or_name: String,
}

fn cmd_read(db: &DB, ReadArgs { addr_or_name }: ReadArgs) -> anyhow::Result<()> {
    let Some(addr) = db.function_from_name(&addr_or_name) else {
        anyhow::bail!("unknown function {addr_or_name:?}")
    };
    let func = db
        .functions
        .get(&addr)
        .ok_or_else(|| anyhow!("unknown function {addr}"))?;
    print!("{}", api::plain_text(func, true));
    Ok(())
}

/// write function: update database with function name, description, etc.
#[derive(argh::FromArgs)]
#[argh(subcommand, name = "write")]
struct WriteArgs {
    #[argh(positional)]
    addr: String,
}

fn cmd_write(db: &mut DB, WriteArgs { addr }: WriteArgs) -> anyhow::Result<()> {
    if addr == "schema" {
        let schema = api::FunctionUpdate::schema();
        println!("{}", serde_json::to_string(&schema).unwrap());
        return Ok(());
    }

    let addr = SegOfs::parse(&addr).map_err(|_| anyhow::anyhow!("bad address {:?}", addr))?;
    let func = db
        .functions
        .get_mut(&addr)
        .ok_or_else(|| anyhow::anyhow!("unknown function {addr}"))?;

    use std::io::Read;
    let mut buf = String::new();
    std::io::stdin().lock().read_to_string(&mut buf)?;
    let update: api::FunctionUpdate = serde_json::from_str(&buf)?;
    update.merge_into(func)?;
    db.write()?;

    Ok(())
}
