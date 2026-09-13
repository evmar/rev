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
    #[argh(positional, from_str_fn(SegOfs::parse))]
    addr: SegOfs,
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

pub async fn run(db: &mut DB, args: Args) -> anyhow::Result<()> {
    label_memory::run(db, &client()?, args.addr).await
}

#[allow(dead_code)]
pub async fn run_old(db: &mut DB, args: Args) -> anyhow::Result<()> {
    let Some(func) = db.functions.get_mut(&args.addr) else {
        anyhow::bail!("no function {}", args.addr);
    };
    annotate_function::run(&client()?, func).await?;
    db.write()?;
    Ok(())
}

fn client() -> anyhow::Result<OpenRouterClient> {
    let apikey = std::env::var("APIKEY").expect("need APIKEY in environment");
    let client = OpenRouterClient::builder().api_key(apikey).build()?;
    Ok(client)
}
