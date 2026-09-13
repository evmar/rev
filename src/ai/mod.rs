mod annotate_function;
mod label_memory;

pub use annotate_function::Var;
use openrouter_rs::{OpenRouterClient, types::ResponseUsage};
use runtime::SegOfs;

use crate::db::DB;

/// ai
#[derive(argh::FromArgs)]
#[argh(subcommand, name = "ai")]
pub struct Args {
    // #[argh(positional, from_str_fn(SegOfs::parse))]
    // addr: SegOfs,
}

fn print_usage(start: std::time::Instant, usage: &ResponseUsage) {
    println!();
    let delta = std::time::Instant::now() - start;
    println!("{}ms", delta.as_millis());
    println!(
        "tokens: {prompt}+{completion}={total}",
        prompt = usage.prompt_tokens,
        completion = usage.completion_tokens,
        total = usage.total_tokens
    );
    if let Some(cost) = usage.cost {
        println!("cost: {:.5}", cost);
    }
}

enum Task {
    AnnotateFunction(SegOfs),
    LabelMemory(SegOfs),
}

impl Task {
    fn desc(&self) -> String {
        match self {
            Task::AnnotateFunction(ip) => format!("annotate function {ip}"),
            Task::LabelMemory(addr) => format!("label memory {addr}"),
        }
    }
}

fn next_queued(db: &DB) -> Option<Task> {
    for func in db.functions.values() {
        if func.desc.is_none() {
            return Some(Task::AnnotateFunction(func.ip));
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

pub async fn run(db: &mut DB, _args: Args) -> anyhow::Result<()> {
    let client = client()?;
    while let Some(task) = next_queued(db) {
        println!("next task: {}", task.desc());
        match task {
            Task::AnnotateFunction(ip) => annotate_function::run(db, &client, ip).await?,
            Task::LabelMemory(addr) => label_memory::run(db, &client, addr).await?,
        }
        db.write()?;
    }
    Ok(())
}

fn client() -> anyhow::Result<OpenRouterClient> {
    let apikey = std::env::var("APIKEY").expect("need APIKEY in environment");
    let client = OpenRouterClient::builder().api_key(apikey).build()?;
    Ok(client)
}
