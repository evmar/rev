use std::collections::HashMap;

use futures_util::StreamExt;
use indoc::indoc;
use openrouter_rs::{
    OpenRouterClient,
    api::chat::{ChatCompletionRequest, Message},
    types::{CompletionsResponse, Role},
};
use runtime::SegOfs;

use crate::{
    db::DB,
    function::{Function, Instr},
};

/// ai
#[derive(argh::FromArgs)]
#[argh(subcommand, name = "ai")]
pub struct Args {
    #[argh(positional, from_str_fn(SegOfs::parse))]
    addr: SegOfs,
}

fn print_usage(start: std::time::Instant, response: &CompletionsResponse) {
    println!();
    let delta = std::time::Instant::now() - start;
    println!("{}ms", delta.as_millis());
    if let Some(usage) = &response.usage {
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
    if let Some(provider) = &response.provider {
        println!("{:#?}", provider);
    }
}

pub async fn run(db: &mut DB, args: Args) -> anyhow::Result<()> {
    let Some(func) = db.functions.get_mut(&args.addr) else {
        anyhow::bail!("no function {}", args.addr);
    };

    let response = call(func).await?;

    let merged = merge(func, &response);
    if merged == 0 {
        anyhow::bail!("no comments found");
    }

    db.write()?;
    Ok(())
}

async fn call(func: &Function) -> anyhow::Result<String> {
    let mut buf = Vec::new();
    func.serialize(&mut buf)?;
    let prompt = std::str::from_utf8(&buf).unwrap();

    let apikey = std::env::var("APIKEY").expect("need APIKEY in environment");

    // Create client with builder pattern
    let client = OpenRouterClient::builder()
        .api_key(apikey)
        //.http_referer("https://yourapp.com")
        //.x_title("My App")
        //.app_categories(["cli-agent"])
        .build()?;

    // Build chat request
    let request = ChatCompletionRequest::builder()
        //.model("google/gemini-3.7-flash")
        .model("z-ai/glm-5.3-flash")
        .messages(vec![
            Message::new(
                Role::System,
                indoc!(
                    "
                    You read the input DOS x86 assembly, and respond with comments on specific addresses
                    about what the code is doing at that address.
                    Do not comment on every line, only higher level comments."
                ),
            ),
            Message::new(
                Role::User,
                indoc!(
                    "
                0823:0cec mov ax,cs
                0823:0cee sub ax,10h
                0823:0cf1 mov ds,ax
                0823:0cf3 mov es,ax
                0823:0cf5 sub ax,ax
                0823:0cf7 sub bx,bx
                0823:0cf9 sub cx,cx
                0823:0cfb sub dx,dx
            "
                ),
            ),
            Message::new(
                Role::Assistant,
                indoc!(
                    "
                0823:0cec set DS and ES to the PSP segment (CS‑0x10)
                0823:0cf5 zero out general‑purpose registers
                "
                ),
            ),
            Message::new(Role::User, prompt),
        ])
        // .temperature(0.7)
        // .max_tokens(500)
        .build()?;

    let start = std::time::Instant::now();
    let mut stream = client.chat().stream(&request).await?;

    let mut full_response = String::new();
    let mut last = None;
    while let Some(result) = stream.next().await {
        if let Ok(response) = result {
            if let Some(content) = response.choices[0].content() {
                print!("{}", content);
                full_response.push_str(&content);
            }
            last = Some(response);
        }
    }
    print_usage(start, &last.unwrap());

    Ok(full_response)
}

fn parse_response(response: &str) -> Vec<(SegOfs, &str)> {
    let mut comments = vec![];
    for line in response.split('\n') {
        if line.is_empty() {
            continue;
        }

        let Some((addr, comment)) = line.split_once(' ') else {
            eprintln!("bad line {line:?}");
            continue;
        };
        let addr = match SegOfs::parse(addr) {
            Ok(addr) => addr,
            Err(err) => {
                eprintln!("bad addr {addr}: {err}");
                continue;
            }
        };
        comments.push((addr, comment));
    }
    comments
}

fn merge(func: &mut Function, response: &str) -> usize {
    let comments = parse_response(response);

    let mut instrs: HashMap<SegOfs, &mut Instr> = func
        .blocks
        .iter_mut()
        .flat_map(|b| {
            b.instrs
                .iter_mut()
                .map(|i| (func.ip.with_ofs(i.iced.ip16()), i))
        })
        .collect();

    let mut found = 0;
    for (addr, comment) in comments {
        if addr.seg != func.ip.seg {
            eprintln!("bad comment address {addr}");
            continue;
        }
        let Some(instr) = instrs.get_mut(&addr) else {
            eprintln!("comment on nonexistent {addr}");
            continue;
        };
        instr.comment = Some(match instr.comment.as_mut() {
            Some(c) => format!("{c}; {comment}"),
            None => comment.into(),
        });
        found += 1;
    }
    found
}
