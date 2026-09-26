use std::collections::VecDeque;

use crate::{
    db::{DB, XRef, xref},
    dis::dis_func,
};

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
        let Some(callees) = &func.callees else {
            continue;
        };
        for xref in callees.iter() {
            let ip = match xref {
                XRef::External(_, ip) => *ip,
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
        dis_func(db, ip);
        let func = db.functions.get_mut(&ip).unwrap();
        xref::update_callees(func, &db.mem, |ip| XRef::External(None, ip));
        db.write()?;
        changed = true;
    }

    if xref::update_all_xrefs(db) {
        changed = true;
    }

    Ok(changed)
}
