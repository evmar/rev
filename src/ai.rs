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

    let merged = merge(func, response);
    if merged == 0 {
        anyhow::bail!("no comments found");
    }

    db.write()?;
    Ok(())
}

async fn call(func: &Function) -> anyhow::Result<Response> {
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

    let schema = serde_json::from_slice(include_bytes!("ai.json")).unwrap();
    let response_format = openrouter_rs::types::ResponseFormat::json_schema("dis", true, schema);

    // Build chat request
    let request = ChatCompletionRequest::builder()
        .model("google/gemini-3.8-flash")
        // .model("z-ai/glm-5.3-flash")
        .response_format(response_format)
        .messages(vec![
            Message::new(
                Role::System,
                indoc!(
                    "
                    You read the input DOS x86 assembly and respond with a description of what the code does.
                    For inline comments, do not comment on every line, but rather summarize blocks of the code."
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

    let response: Response = serde_json::from_str(&full_response)?;
    Ok(response)
}

fn merge(func: &mut Function, response: Response) -> usize {
    func.name = Some(response.name);
    func.desc = Some(response.desc);

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
    for InlineComment { addr, text } in response.inline_comments {
        let Ok(addr) = SegOfs::parse(&addr) else {
            eprintln!("bad comment address {addr}");
            continue;
        };
        if addr.seg != func.ip.seg {
            eprintln!("bad comment address {addr}");
            continue;
        }
        let Some(instr) = instrs.get_mut(&addr) else {
            eprintln!("comment on nonexistent {addr}");
            continue;
        };
        instr.comment = Some(match instr.comment.as_mut() {
            Some(c) => format!("{c}; {text}"),
            None => text,
        });
        found += 1;
    }
    found
}

#[derive(serde::Deserialize, Debug)]
struct Response {
    name: String,
    desc: String,
    inline_comments: Vec<InlineComment>,
}
#[derive(serde::Deserialize, Debug)]
struct InlineComment {
    addr: String,
    text: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse() {
        let response = r#"
            {
              "desc": "Saves the entire Interrupt Vector Table (IVT) of 1024 bytes (512 words) from segment 0000:0000 into a buffer located within the code segment at CS:03FC. All modified registers are preserved on the stack.",
              "inline_comments": [
                {
                  "addr": "0823:09e5",
                  "text": "Save general-purpose and segment registers to the stack"
                },
                {
                  "addr": "0823:09ee",
                  "text": "Copy 512 words (1024 bytes) from 0000:0000 (IVT) to CS:03FC"
                },
                {
                  "addr": "0823:09fc",
                  "text": "Restore saved registers and return"
                }
              ],
              "name": "backup_ivt"
            }
            "#;
        let _response: Response = serde_json::from_str(response).unwrap();
    }
}
