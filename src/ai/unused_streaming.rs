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

    let mut content = String::new();
    let mut stream = self.client.chat().stream_tool_aware(&request).await?;
    while let Some(event) = stream.next().await {
        use openrouter_rs::types::StreamEvent;
        match event {
            StreamEvent::Error(err) => panic!("err {err}"),
            StreamEvent::ContentDelta(c) => {
                content.push_str(&c);
            }
            StreamEvent::ReasoningDelta(reasoning) => print!("{reasoning}"),
            StreamEvent::ReasoningDetailsDelta(_) => {}
            StreamEvent::Done {
                tool_calls,
                finish_reason,
                usage,
                ..
            } => {
                if let Some(usage) = usage {
                    self.usage.add(&usage);
                }
                if let Some(finish) = finish_reason {
                    use openrouter_rs::types::FinishReason;
                    match finish {
                        FinishReason::ToolCalls => {
                            self.messages
                                .push(Message::assistant_with_tool_calls("", tool_calls.clone()));
                            for call in tool_calls.iter() {
                                if call.is_tool::<GetFunctionParams>() {
                                    let params = call.parse_params::<GetFunctionParams>()?;
                                    let func = self
                                        .db
                                        .functions
                                        .get(&SegOfs::parse(&params.addr).unwrap())
                                        .unwrap();
                                    println!(
                                        "Reading function {} ({})",
                                        params.addr,
                                        func.name.as_deref().unwrap_or("unknown name")
                                    );
                                    let mut buf = String::new();
                                    func.serialize(&mut buf)?;
                                    self.messages.push(Message::tool_response(call.id(), buf));
                                } else {
                                    panic!();
                                }
                            }
                        }
                        FinishReason::Stop => {
                            self.usage.print();
                            return Ok(Some(content));
                        }
                        _ => todo!("finish {:?}", finish),
                    }
                }
            }
            _ => todo!(),
        }
    }
    Ok(None)
}
