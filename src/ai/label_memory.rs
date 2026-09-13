use openrouter_rs::{
    Message, OpenRouterClient,
    api::chat::ChatCompletionRequest,
    types::{ResponseFormat, ResponseUsage, Role, TypedTool},
};
use runtime::SegOfs;
use schemars::schema_for;

use crate::{db::DB, memory::MemoryLocation};

#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
struct GetFunctionParams {
    /// address of function to get
    addr: String,
}

impl TypedTool for GetFunctionParams {
    fn name() -> &'static str {
        "get_function"
    }

    fn description() -> &'static str {
        "get a function's name, description, and code"
    }
}

impl GetFunctionParams {
    fn call(db: &DB, params: &GetFunctionParams) -> anyhow::Result<String> {
        let addr = SegOfs::parse(&params.addr)
            .map_err(|_| anyhow::anyhow!("bad address {:?}", params.addr))?;
        let func = db
            .functions
            .get(&addr)
            .ok_or_else(|| anyhow::anyhow!("unknown function {addr}"))?;
        println!(
            "Reading function {} ({})",
            params.addr,
            func.name.as_deref().unwrap_or("unknown name")
        );
        let mut buf = String::new();
        func.serialize(&mut buf)?;
        Ok(buf)
    }
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct Response {
    /// short name for the memory location
    pub name: String,
    /// description of what the memory holds
    pub desc: String,
    /// value type as expressed in Rust, e.g. `u32` or `CStr`
    pub typ: String,
}

pub async fn run(db: &mut DB, client: &OpenRouterClient, addr: SegOfs) -> anyhow::Result<()> {
    let mut functions = vec![];
    for func in db.functions.values() {
        for block in func.blocks.iter() {
            for instr in block.instrs.iter() {
                if let Some(Ok(mem_ref)) = instr.memory {
                    if mem_ref == addr {
                        functions.push(func.ip);
                    }
                }
            }
        }
    }

    let Response { name, desc, typ } = call(db, client, functions, addr).await?;
    println!("{addr}: {name} {typ}");
    println!("{desc}");
    db.memory
        .entries
        .insert(addr, MemoryLocation { name, desc, typ });
    Ok(())
}

async fn call(
    db: &mut DB,
    client: &OpenRouterClient,
    functions: Vec<SegOfs>,
    addr: SegOfs,
) -> anyhow::Result<Response> {
    let prompt = format!(
        indoc::indoc! {"
            analyze usage of memory at address {addr} to figure out its name, description, and type.
            it is used by these functions: {functions}
            once you have a good guess, you don't need to read every last function.
        "},
        addr = addr,
        functions = functions
            .iter()
            .map(|ip| ip.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    );

    let mut agent_loop = AgentLoop {
        db,
        client,
        messages: vec![
            Message::new(Role::System, "you analyze x86 assembly for DOS executables"),
            Message::new(Role::User, prompt),
        ],
        usage: Default::default(),
    };

    let content = agent_loop.run().await?;
    let response: Response = serde_json::from_str(&content)?;
    Ok(response)
}

#[derive(Default)]
struct Usage {
    prompt: u32,
    completion: u32,
    total: u32,
    cost: f64,
}

impl Usage {
    fn add(&mut self, usage: &ResponseUsage) {
        self.prompt += usage.prompt_tokens;
        self.completion = usage.completion_tokens;
        self.total = usage.total_tokens;
        if let Some(cost) = usage.cost {
            self.cost += cost;
        }
    }

    fn print(&self) {
        println!(
            "{}+{}={} / ${:.5}",
            self.prompt, self.completion, self.total, self.cost
        );
    }
}

struct AgentLoop<'client> {
    db: &'client mut DB,
    client: &'client OpenRouterClient,
    messages: Vec<Message>,
    usage: Usage,
}

impl<'client> AgentLoop<'client> {
    async fn run(&mut self) -> anyhow::Result<String> {
        for _ in 0..20 {
            if let Some(content) = self.run_one().await? {
                return Ok(content);
            }
        }
        anyhow::bail!("no response within turn limit")
    }

    async fn run_one(&mut self) -> anyhow::Result<Option<String>> {
        let request = ChatCompletionRequest::builder()
            .model("google/gemini-3.8-flash")
            .typed_tool::<GetFunctionParams>()
            .response_format(ResponseFormat::json_schema(
                "label",
                true,
                schema_for!(Response).to_value(),
            ))
            .messages(self.messages.clone())
            .build()?;

        let resp = self.client.chat().create(&request).await?;
        if let Some(usage) = &resp.usage {
            self.usage.add(&usage);
        }

        let choice = &resp.choices[0];

        if let Some(reasoning) = choice.reasoning() {
            print!("{reasoning}")
        }

        if let Some(tool_calls) = choice.tool_calls() {
            self.messages
                .push(Message::assistant_with_tool_calls("", tool_calls.to_vec()));
            for call in tool_calls.iter() {
                if call.is_tool::<GetFunctionParams>() {
                    let params = call.parse_params::<GetFunctionParams>()?;
                    let out = match GetFunctionParams::call(self.db, &params) {
                        Ok(out) => out,
                        Err(err) => format!("error: {err}"),
                    };
                    self.messages.push(Message::tool_response(call.id(), out));
                } else {
                    panic!();
                }
            }
        }

        if let Some(reason) = choice.finish_reason() {
            use openrouter_rs::types::FinishReason;
            match reason {
                FinishReason::ToolCalls => {}
                FinishReason::Stop => {
                    self.usage.print();
                    return Ok(choice.content().map(|s| s.to_owned()));
                }
                _ => todo!("finish {:?}", reason),
            }
        }
        Ok(None)
    }
}
