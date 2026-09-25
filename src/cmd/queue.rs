use std::path::PathBuf;

use runtime::SegOfs;

use crate::db::DB;
use anyhow::anyhow;

/// agentically process queue
#[derive(argh::FromArgs)]
#[argh(subcommand, name = "queue")]
pub struct Args {
    /// run one task
    #[argh(option)]
    count: Option<usize>,
}

pub enum Task {
    DescribeFunction(SegOfs),
    LabelMemory(SegOfs),
}

pub fn next_task(db: &DB) -> Option<Task> {
    for func in db.functions.values() {
        if func.callers.is_none() && func.callees.is_none() {
            continue;
        }

        if func.desc.is_none() {
            return Some(Task::DescribeFunction(func.ip));
        }

        for block in func.blocks.iter() {
            for instr in block.instrs.iter() {
                if let Some(Ok(addr)) = instr.memory {
                    if !db.memory.entries.contains_key(&addr) {
                        return Some(Task::LabelMemory(addr));
                    }
                }
            }
        }
    }

    None
}

pub fn codex_task(_db: &DB, task: &Task) -> anyhow::Result<()> {
    let task = match task {
        Task::DescribeFunction(ip) => {
            println!("task: describe function {ip}");
            format!("analyze {ip} and write analysis back to database")
        }
        Task::LabelMemory(_addr) => todo!(),
    };

    let cmd = "./agent.sh";
    let mut process = std::process::Command::new("./agent.sh")
        .current_dir("codex")
        .arg(task)
        .spawn()
        .map_err(|err| anyhow!("{cmd}: {err}"))?;
    let _exit = process.wait()?;
    Ok(())
}

pub fn run(path: PathBuf, args: Args) -> anyhow::Result<()> {
    let count = args.count.unwrap_or(1000);
    for _ in 0..count {
        let db = DB::load(path.clone())?;
        let Some(task) = next_task(&db) else {
            return Ok(());
        };
        codex_task(&db, &task)?;
    }
    Ok(())
}
