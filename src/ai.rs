use futures_util::StreamExt;
use indoc::indoc;
use openrouter_rs::{
    OpenRouterClient,
    api::chat::{ChatCompletionRequest, Message},
    types::{CompletionsResponse, Role},
};

/// ai
#[derive(argh::FromArgs)]
#[argh(subcommand, name = "ai")]
pub struct Args {}

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

pub async fn call() -> Result<(), Box<dyn std::error::Error>> {
    let prompt = String::from_utf8(std::fs::read("txt").unwrap()).unwrap();

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
                    You are a DOS and x86 expert.
                    You read the input assembly, and respond with comments on specific addresses
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

    let mut last = None;
    while let Some(result) = stream.next().await {
        if let Ok(response) = result {
            if let Some(content) = response.choices[0].content() {
                print!("{}", content);
            }
            last = Some(response);
        }
    }
    print_usage(start, &last.unwrap());

    Ok(())
}
