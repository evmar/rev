use std::collections::VecDeque;

use runtime::SegOfs;

use crate::db::{DB, Function};

/// crawl references
#[derive(argh::FromArgs)]
#[argh(subcommand, name = "crawl")]
pub struct Args {}

pub fn run(db: &mut DB, _args: Args) -> anyhow::Result<()> {
    if crawl(db)? {
        db.write()?;
    }
    Ok(())
}

pub fn crawl(db: &mut DB) -> anyhow::Result<bool> {
    fn enqueue_callees(queue: &mut VecDeque<SegOfs>, func: &Function) {
        for ip in func.gather_callees() {
            if queue.contains(&ip) {
                continue;
            }
            queue.push_back(ip);
        }
    }

    let mut queue = VecDeque::new();
    for func in db.functions.values() {
        enqueue_callees(&mut queue, func);
    }

    let mut changed = false;
    while let Some(ip) = queue.pop_front() {
        if db.functions.contains_key(&ip) {
            continue;
        };
        println!("visiting {ip}");
        let func = db.import_function(ip);
        enqueue_callees(&mut queue, func);
        changed = true;
    }

    Ok(changed)
}
