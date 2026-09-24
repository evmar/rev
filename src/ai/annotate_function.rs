use futures_util::StreamExt;
use openrouter_rs::{Message, OpenRouterClient, api::chat::ChatCompletionRequest, types::Role};
use runtime::SegOfs;

use crate::{api, db::DB, function::Function};

pub async fn run(db: &mut DB, client: &OpenRouterClient, ip: SegOfs) -> anyhow::Result<()> {
    let Some(func) = db.functions.get_mut(&ip) else {
        anyhow::bail!("no function {}", ip);
    };
    let update = call(client, func).await?;
    update.merge_into(func)?;
    Ok(())
}

async fn call(client: &OpenRouterClient, func: &Function) -> anyhow::Result<api::FunctionUpdate> {
    let mut prompt = format!(
        "analyze this function. provide annotations such as comments only when they add information\n\n"
    );

    func.clone_bare().serialize(&mut prompt)?;

    let response_format = openrouter_rs::types::ResponseFormat::json_schema(
        "dis",
        true,
        serde_json::to_value(api::FunctionUpdate::schema())?,
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
    let update: api::FunctionUpdate = serde_json::from_str(&content)?;
    Ok(update)
}
