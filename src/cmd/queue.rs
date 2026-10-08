use std::path::PathBuf;

use runtime::SegOfs;

use crate::{ai, db::DB};

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

pub fn next_task(db: &mut DB) -> Option<Task> {
    for func in db.functions.values() {
        if db.xrefs.callers.get(&func.ip).is_none() && db.xrefs.callees.get(&func.ip).is_none() {
            continue;
        }

        if func.desc.is_none() {
            return Some(Task::DescribeFunction(func.ip));
        }

        for block in func.blocks.iter() {
            for instr in block.instrs.iter() {
                if let Some(Ok(addr)) = func
                    .annotations
                    .get(&block.ip.with_ofs(instr.ip16()))
                    .and_then(|a| a.memory.as_ref())
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
            indoc::formatdoc! {"
                ## high-level program description
                {desc}

                ## task
                analyze function {ip} and write analysis of its name, arguments, etc.
                back to database using write_function().

                if while analyzing the function you discover information about any
                static memory locations used by {ip}, label them with label_memory().

                you may inspect other functions using get_function() when necessary for
                understanding {ip}.  do not speculate about {ip} while an important
                dependency remains unresolved.  stop once additional exploration will not
                improve the understanding of {ip}.
                ",
                desc = db.exe.description.as_deref().unwrap_or("none"),
            }
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
        let Some(task) = next_task(&mut db) else {
            return Ok(());
        };
        run_task(&mut db, &task).await?;
    }
    Ok(())
}
