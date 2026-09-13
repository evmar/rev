use std::collections::HashMap;

use futures_util::StreamExt;
use openrouter_rs::{Message, OpenRouterClient, api::chat::ChatCompletionRequest, types::Role};
use runtime::SegOfs;

use crate::{
    db::DB,
    function::{Function, Instr},
};

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct Response {
    /// short function name, a guess at what the function does
    pub name: String,
    /// brief description of function: one-liner summary of what function is for
    pub desc: String,
    /// longer description of function: two or three sentences
    pub details: String,
    /// function parameters, both registers and from stack
    pub parameters: Vec<Var>,
    /// return value, if any
    #[serde(rename = "return")]
    pub ret: Option<Var>,
    /// if it's the target of a local jmp, a name for the label for the instruction
    pub labels: Vec<Annotation>,
    /// comment describing what the instruction or following block does
    pub inline_comments: Vec<Annotation>,
}

#[derive(serde::Deserialize, Debug, schemars::JsonSchema)]
struct Annotation {
    pub addr: String,
    pub text: String,
}

impl Annotation {
    fn parse(self) -> anyhow::Result<(SegOfs, String)> {
        let addr = SegOfs::parse(&self.addr)
            .map_err(|err| anyhow::anyhow!("parsing {addr}: {err}", addr = self.addr))?;
        Ok((addr, self.text))
    }
}

#[derive(serde::Deserialize, serde::Serialize, Debug, ts_rs::TS, schemars::JsonSchema)]
#[ts(export_to = "../../web/src/bindings/")]
pub struct Var {
    /// an identifier name for the variable within the function
    pub name: String,
    /// the parameter's storage, either a register like `cx` or an stack offset like `[sp+4]`
    pub value: String,
    /// value type as expressed in Rust, e.g. `u32` or `CStr`
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

pub async fn run(db: &mut DB, client: &OpenRouterClient, ip: SegOfs) -> anyhow::Result<()> {
    let Some(func) = db.functions.get_mut(&ip) else {
        anyhow::bail!("no function {}", ip);
    };
    let resp = call(client, func).await?;
    merge(func, resp)?;
    Ok(())
}

async fn call(client: &OpenRouterClient, func: &Function) -> anyhow::Result<Response> {
    let mut prompt = format!(
        "analyze this function. provide annotations such as comments only when they add information\n\n"
    );

    func.clone_bare().serialize(&mut prompt)?;

    let response_format = openrouter_rs::types::ResponseFormat::json_schema(
        "dis",
        true,
        serde_json::to_value(response_schema())?,
    );

    let messages = vec![
        Message::new(Role::System, "you analyze DOS x86 assembly"),
        Message::new(Role::User, prompt),
    ];

    let request = ChatCompletionRequest::builder()
        .model("google/gemini-3.8-flash")
        .response_format(response_format)
        .messages(messages)
        .build()?;

    let start = std::time::Instant::now();
    let mut stream = client.chat().stream(&request).await?;

    let mut content = String::new();
    while let Some(resp) = stream.next().await {
        let resp = resp?;
        let choice = &resp.choices[0];
        if let Some(reasoning) = choice.reasoning() {
            println!("{reasoning}");
        }
        if let Some(c) = choice.content() {
            content.push_str(c);
        }
        if let Some(usage) = &resp.usage {
            super::print_usage(start, &usage);
        }
    }
    let response: Response = serde_json::from_str(&content)?;
    Ok(response)
}

fn merge(func: &mut Function, response: Response) -> anyhow::Result<()> {
    func.name = Some(response.name);
    func.desc = Some(response.desc);
    func.details = Some(response.details);

    func.params = if response.parameters.is_empty() {
        None
    } else {
        Some(response.parameters)
    };
    func.ret = response.ret;

    let mut instrs: HashMap<SegOfs, &mut Instr> = Default::default();
    for block in func.blocks.iter_mut() {
        for instr in block.instrs.iter_mut() {
            instr.label = None;
            instr.comment = None;
            instrs.insert(func.ip.with_ofs(instr.iced.ip16()), instr);
        }
    }

    for label in response.labels {
        let (addr, label) = label.parse()?;
        let Some(instr) = instrs.get_mut(&addr) else {
            anyhow::bail!("label on nonexistent {addr}");
        };
        instr.label = Some(label);
    }

    for comment in response.inline_comments {
        let (addr, comment) = comment.parse()?;
        let Some(instr) = instrs.get_mut(&addr) else {
            anyhow::bail!("comment on nonexistent {addr}");
        };
        instr.comment = Some(comment);
    }

    Ok(())
}
