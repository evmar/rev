use std::collections::VecDeque;

use crate::db::{DB, XRef};

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
    let mut queue = VecDeque::new();
    for func in db.functions.values() {
        for xref in func.annotations.values().filter_map(|a| a.jmp.as_ref()) {
            let ip = match xref {
                XRef::External(ip) => *ip,
                _ => continue,
            };
            if db.functions.contains_key(&ip) {
                continue;
            }
            if queue.contains(&ip) {
                continue;
            }
            queue.push_back(ip);
        }
    }

    let mut changed = false;
    while let Some(ip) = queue.pop_front() {
        if db.functions.contains_key(&ip) {
            continue;
        };
        println!("visiting {ip}");
        db.import_function(ip);
        db.write()?;
        changed = true;
    }

    Ok(changed)
}
