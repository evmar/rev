use openrouter_rs::{
    Message, OpenRouterClient,
    api::chat::ChatCompletionRequest,
    types::{ResponseUsage, Role},
};

use crate::{
    ai::tool::{GetFunctionTool, ReadMemoryTool, WriteFunctionTool},
    db::DB,
};

pub fn client() -> anyhow::Result<OpenRouterClient> {
    let apikey = std::env::var("APIKEY").expect("need APIKEY in environment");
    let client = OpenRouterClient::builder().api_key(apikey).build()?;
    Ok(client)
}

#[allow(unused)]
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

pub struct AgentLoop<'client> {
    db: &'client mut DB,
    client: &'client OpenRouterClient,
    messages: Vec<Message>,
    usage: Usage,
}

impl<'client> AgentLoop<'client> {
    pub fn new(db: &'client mut DB, client: &'client OpenRouterClient, prompt: String) -> Self {
        AgentLoop {
            db,
            client,
            messages: vec![
                Message::new(Role::System, "you analyze x86 assembly for DOS executables"),
                Message::new(Role::User, prompt),
            ],
            usage: Default::default(),
        }
    }

    pub async fn run(&mut self) -> anyhow::Result<String> {
        for i in 1..100 {
            println!("agent turn {i}:");
            if let Some(content) = self.run_one().await? {
                self.usage.print();
                return Ok(content);
            }
        }
        anyhow::bail!("no response within turn limit")
    }

    async fn run_one(&mut self) -> anyhow::Result<Option<String>> {
        let as_text = serde_json::to_string(&self.messages).unwrap();
        std::fs::write("ai-trace.json", as_text.as_bytes()).unwrap();

        let request = ChatCompletionRequest::builder()
            .model("google/gemini-3.8-flash")
            .typed_tool::<GetFunctionTool>()
            .typed_tool::<ReadMemoryTool>()
            .typed_tool::<WriteFunctionTool>()
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
                let out = if call.is_tool::<GetFunctionTool>() {
                    let params = call.parse_params::<GetFunctionTool>()?;
                    GetFunctionTool::call(self.db, params)
                } else if call.is_tool::<ReadMemoryTool>() {
                    let params = call.parse_params::<ReadMemoryTool>()?;
                    ReadMemoryTool::call(self.db, params)
                } else if call.is_tool::<WriteFunctionTool>() {
                    let params = call.parse_params::<WriteFunctionTool>()?;
                    WriteFunctionTool::call(self.db, params)
                } else {
                    panic!("unexpected call {call:?}");
                };
                let out = match out {
                    Ok(out) => out,
                    Err(err) => {
                        println!("tool failed: {err}");
                        format!("error: {err}")
                    }
                };
                self.messages.push(Message::tool_response(call.id(), out))
            }
        }

        if let Some(reason) = choice.finish_reason() {
            use openrouter_rs::types::FinishReason;
            match reason {
                FinishReason::ToolCalls => {}
                FinishReason::Stop => {
                    println!("stop");
                    return Ok(choice.content().map(|s| s.to_owned()));
                }
                _ => todo!("finish {:?}", reason),
            }
        }
        Ok(None)
    }
}
