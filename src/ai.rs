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

    let schema = serde_json::to_value(response_schema())?;
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

    func.params = if response.parameters.is_empty() {
        None
    } else {
        Some(response.parameters)
    };
    func.ret = response.ret;

    let mut instrs: HashMap<SegOfs, &mut Instr> = Default::default();
    for block in func.blocks.iter_mut() {
        for instr in block.instrs.iter_mut() {
            instr.comment = None;
            instrs.insert(func.ip.with_ofs(instr.iced.ip16()), instr);
        }
    }

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

#[derive(serde::Deserialize, Debug, schemars::JsonSchema)]
#[schemars(rename = "dis")]
struct Response {
    /// short function name, a guess at what the function does
    pub name: String,
    /// top-level description of function, a few lines of text
    pub desc: String,
    /// function parameters, both registers and from stack
    pub parameters: Vec<Var>,
    /// return value, if any
    #[serde(rename = "return")]
    pub ret: Option<Var>,
    /// inline comments on code, describing what each block of code does
    pub inline_comments: Vec<InlineComment>,
}

#[derive(serde::Deserialize, Debug, schemars::JsonSchema)]
struct InlineComment {
    pub addr: String,
    pub text: String,
}

#[derive(serde::Deserialize, serde::Serialize, Debug, ts_rs::TS, schemars::JsonSchema)]
#[ts(export_to = "../web/src/bindings/")]
pub struct Var {
    /// an identifier name for the variable within the function
    pub name: String,
    /// the parameter's value, either a register or an stack offset like [sp+4]
    pub value: String,
    /// value type, e.g. u32 or cstr
    #[serde(rename = "type")]
    pub typ: String,
    /// human-readable description
    pub desc: String,
}

fn response_schema() -> schemars::Schema {
    schemars::generate::SchemaSettings::default()
        .with(|settings| settings.inline_subschemas = true)
        .into_generator()
        .into_root_schema_for::<Response>()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn response_schema_matches_response_fields() {
        let schema = response_schema();
        println!("{}", serde_json::to_string_pretty(&schema).unwrap());
        let schema = serde_json::to_value(schema).unwrap();
        assert_eq!(schema["title"], "dis");
        let required = schema["required"].as_array().unwrap();
        for field in ["name", "desc", "parameters", "inline_comments"] {
            assert!(required.contains(&serde_json::json!(field)));
        }
        assert!(!required.contains(&serde_json::json!("description")));
        assert!(schema["properties"]["return"].is_object());
        assert!(schema["properties"].get("ret").is_none());
        assert_eq!(
            schema["properties"]["parameters"]["items"]["properties"]["type"]["type"],
            "string"
        );
        assert!(!serde_json::to_string(&schema).unwrap().contains("\"$ref\""));
        assert_eq!(
            schema["properties"]["name"]["description"],
            "short function name, a guess at what the function does"
        );
        let response: Response = serde_json::from_value(serde_json::json!({
            "name": "example", "desc": "description", "parameters": [],
            "return": null, "inline_comments": []
        }))
        .unwrap();
        assert!(response.ret.is_none());
    }
}
