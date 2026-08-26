use futures_util::StreamExt;
use openrouter_rs::{
    OpenRouterClient,
    api::chat::{ChatCompletionRequest, Message},
    types::Role,
};

pub async fn call() -> Result<(), Box<dyn std::error::Error>> {
    let apikey = std::env::var("APIKEY").unwrap();

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
        .model("openrouter/free")
        .messages(vec![
            Message::new(Role::System, "You are a helpful assistant"),
            Message::new(
                Role::User,
                "Explain Rust ownership in simple terms, in one paragraph",
            ),
        ])
        // .temperature(0.7)
        // .max_tokens(500)
        .build()?;

    let mut stream = client.chat().stream(&request).await?;

    let now = std::time::Instant::now();
    while let Some(result) = stream.next().await {
        if let Ok(response) = result {
            if let Some(content) = response.choices[0].content() {
                print!("{}", content);
            }
            if let Some(usage) = response.usage {
                let delta = std::time::Instant::now() - now;
                println!("{}ms", delta.as_millis());
                println!("{:#?}", usage);
                if let Some(provider) = response.provider {
                    println!("{:#?}", provider);
                }
            }
        }
    }

    Ok(())
}
