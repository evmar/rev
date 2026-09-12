// Unused leftover code while tinkering with agent

/*
impl TypedTool for DescribeFunctionParams {
    fn name() -> &'static str {
        "describe_function"
    }

    fn description() -> &'static str {
        "update the description of a function"
    }
}
*/
async fn call(func: &Function) -> anyhow::Result<()> {
    let mut prompt =
        format!("analyze this function and update its description using describe_function()\n\n");
    func.serialize(&mut prompt)?;

    let schema = serde_json::to_value(response_schema())?;
    //let response_format = openrouter_rs::types::ResponseFormat::json_schema("dis", true, schema);

    let mut messages = vec![
        Message::new(Role::System, indoc!("you analyze DOS x86 assembly")),
        Message::new(Role::User, prompt),
    ];

    for _ in 0..5 {
        let request = ChatCompletionRequest::builder()
            .model("google/gemini-3.8-flash")
            // .model("z-ai/glm-5.3-flash")
            //  .response_format(response_format)
            .typed_tool::<DescribeFunctionParams>()
            .messages(messages.clone())
            // .temperature(0.7)
            // .max_tokens(500)
            .build()?;

        let start = std::time::Instant::now();
        let mut stream = client.chat().stream_tool_aware(&request).await?;

        while let Some(event) = stream.next().await {
            use openrouter_rs::types::StreamEvent;
            match event {
                StreamEvent::Error(err) => panic!("err {err}"),
                StreamEvent::ContentDelta(content) => println!("content {content:?}"),
                StreamEvent::ReasoningDelta(reasoning) => println!("{reasoning}"),
                StreamEvent::ReasoningDetailsDelta(_) => {}
                StreamEvent::Done {
                    tool_calls,
                    finish_reason,
                    usage,
                    ..
                } => {
                    if let Some(usage) = usage {
                        print_usage(start, &usage);
                    }
                    if let Some(finish) = finish_reason {
                        use openrouter_rs::types::FinishReason;
                        match finish {
                            FinishReason::ToolCalls => {
                                for tool_call in tool_calls.iter() {
                                    println!("call {:?}", tool_call);
                                    messages.push(Message::tool_response(tool_call.id(), "ok"));
                                }
                            }
                            FinishReason::Stop => todo!(),
                            FinishReason::Length => todo!(),
                            FinishReason::ContentFilter => todo!(),
                            FinishReason::Error => todo!(),
                            FinishReason::Other(_) => todo!(),
                            _ => todo!(),
                        }
                    }
                }
                _ => todo!(),
            }
        }
    }
    Ok(())
}
