mod annotate_function;
mod label_memory;
mod read_memory;

use crate::db::DB;
use openrouter_rs::{OpenRouterClient, types::ResponseUsage};

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

async fn run_next_queued(db: &mut DB, client: &OpenRouterClient) -> anyhow::Result<bool> {
    for func in db.functions.values() {
        if func.desc.is_none() {
            println!("task: annotate function {}", func.ip);
            annotate_function::run(db, client, func.ip).await?;
            return Ok(true);
        }

        for block in func.blocks.iter() {
            for instr in block.instrs.iter() {
                if let Some(Ok(addr)) = instr.memory {
                    if !db.memory.entries.contains_key(&addr) {
                        println!("task: label memory {addr}");
                        label_memory::run(db, &client, addr).await?;
                        return Ok(true);
                    }
                }
            }
        }
    }
    Ok(false)
}

pub async fn run(db: &mut DB, _args: Args) -> anyhow::Result<()> {
    let client = client()?;
    while run_next_queued(db, &client).await? {
        db.write()?;
    }
    Ok(())
}

fn client() -> anyhow::Result<OpenRouterClient> {
    let apikey = std::env::var("APIKEY").expect("need APIKEY in environment");
    let client = OpenRouterClient::builder().api_key(apikey).build()?;
    Ok(client)
}
