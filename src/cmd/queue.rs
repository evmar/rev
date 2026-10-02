use std::path::PathBuf;

use runtime::SegOfs;

use crate::{ai, crawl::crawl, db::DB};

/// agentically process queue
#[derive(argh::FromArgs)]
#[argh(subcommand, name = "queue")]
pub struct Args {
    /// run a limited number of tasks
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
                if let Some(Ok(addr)) = func
                    .annotations
                    .get(&block.ip.with_ofs(instr.iced.ip16()))
                    .map(|a| a.memory.as_ref())
                    .flatten()
                {
                    if !db.memory.entries.contains_key(addr) {
                        return Some(Task::LabelMemory(*addr));
                    }
                }
            }
        }
    }

    None
}

pub async fn run_task(db: &mut DB, task: &Task) -> anyhow::Result<()> {
    let prompt = match task {
        Task::DescribeFunction(ip) => {
            println!("task: describe function {ip}");
            format!(
                "analyze function {ip} and write analysis back to database using write_function()"
            )
        }
        Task::LabelMemory(_addr) => todo!(),
    };
    // codex::run(&prompt)?;
    ai::run(db, prompt).await?;
    db.write()?;

    Ok(())
}

pub async fn run(path: PathBuf, args: Args) -> anyhow::Result<()> {
    let count = args.count.unwrap_or(1000);
    for _ in 0..count {
        let mut db = DB::load(path.clone())?;
        if crawl(&mut db)? {
            db.write()?;
            println!("crawl found new code, looping");
            continue;
        }
        let Some(task) = next_task(&db) else {
            return Ok(());
        };
        run_task(&mut db, &task).await?;
    }
    Ok(())
}
