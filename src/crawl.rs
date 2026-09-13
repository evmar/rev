use std::collections::VecDeque;

use crate::{
    db::DB,
    dis::dis_func,
    xref::{self, XRef},
};

/// crawl references
#[derive(argh::FromArgs)]
#[argh(subcommand, name = "crawl")]
pub struct Args {}

pub async fn run(db: &mut DB, _args: Args) -> anyhow::Result<()> {
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

    while let Some(ip) = queue.pop_front() {
        if db.functions.contains_key(&ip) {
            continue;
        };
        println!("visiting {ip}");
        let func = dis_func(db, ip);
        xref::update_callees(func, |ip| XRef::External(None, ip));
        db.write()?;
    }

    xref::update_all_xrefs(db);
    db.write()?;

    Ok(())
}
