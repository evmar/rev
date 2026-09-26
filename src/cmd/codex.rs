//! Run `codex exec` subprocess, parsing its JSON output.

use std::io::BufReader;

use anyhow::anyhow;

#[derive(serde::Deserialize, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ItemStartedKind {
    CommandExecution { command: String },
}

#[derive(serde::Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
struct ItemStarted {
    // id: String,
    #[serde(flatten)]
    kind: ItemStartedKind,
}

#[derive(serde::Deserialize, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ItemCompletedKind {
    AgentMessage {
        text: String,
    },
    CommandExecution {
        command: String,
        exit_code: Option<i32>,
    },
    Error {
        message: String,
    },
}

#[derive(serde::Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
struct ItemCompleted {
    // id: String,
    #[serde(flatten)]
    kind: ItemCompletedKind,
}

#[derive(serde::Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
struct Usage {
    input_tokens: u32,
    cached_input_tokens: u32,
    cache_write_input_tokens: u32,
    output_tokens: u32,
    reasoning_output_tokens: u32,
}

#[derive(serde::Deserialize, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Event {
    #[serde(rename = "thread.started")]
    ThreadStarted,
    #[serde(rename = "turn.started")]
    TurnStarted,
    #[serde(rename = "turn.completed")]
    TurnCompleted {
        usage: Usage,
    },
    #[serde(rename = "item.started")]
    ItemStarted {
        item: ItemStarted,
    },
    #[serde(rename = "item.completed")]
    ItemCompleted {
        item: ItemCompleted,
    },
    Error {
        message: String,
    },
}

pub fn run(cmd: &str) -> anyhow::Result<()> {
    let mut process = std::process::Command::new("./agent.sh")
        .current_dir("codex")
        .stdout(std::process::Stdio::piped())
        .arg(cmd)
        .spawn()
        .map_err(|err| anyhow!("{cmd}: {err}"))?;
    let stdout = process
        .stdout
        .take()
        .ok_or_else(|| anyhow!("failed to take stdout"))?;
    let reader = BufReader::new(stdout);
    use std::io::BufRead;
    for line in reader.lines() {
        let line = line?;
        let event = match serde_json::from_str::<Event>(&line) {
            Ok(e) => e,
            Err(err) => {
                println!("output: {line:?}");
                println!("failed to parse: {err}");
                continue;
            }
        };
        match event {
            Event::ThreadStarted => println!("thread started"),
            Event::TurnStarted => println!("turn started"),
            Event::TurnCompleted { usage } => println!("turn completed {usage:?}"),
            Event::ItemStarted { item } => match item.kind {
                ItemStartedKind::CommandExecution { command } => {
                    println!("run {command}");
                }
            },
            Event::ItemCompleted { item } => match item.kind {
                ItemCompletedKind::AgentMessage { text } => println!("agent: {text}"),
                ItemCompletedKind::CommandExecution { command, exit_code } => {
                    println!("command: {command}\nexit: {exit_code:?}")
                }
                ItemCompletedKind::Error { message } => println!("error: {message}"),
            },
            Event::Error { message } => println!("error: {message}"),
        }
    }

    let _exit = process.wait()?;
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn parse() {
        let input = r#"{"type":"thread.started","thread_id":"01a0daa8-b51e-77b2-990c-c6e250bbddca"}
        {"type":"turn.started"}
        {"type":"item.completed","item":{"id":"item_0","type":"agent_message","text":"I’ll read the function and database schema, analyze its behavior, and write the findings back."}}
        {"type":"item.started","item":{"id":"item_1","type":"command_execution","command":"/bin/zsh -lc './rev --project ~/projects/rev/sbtalker func read 0d72:5ca6'","aggregated_output":"","exit_code":null,"status":"in_progress"}}
         {"type":"turn.completed","usage":{"input_tokens":156413,"cached_input_tokens":131328,"cache_write_input_tokens":0,"output_tokens":1568,"reasoning_output_tokens":392}}
        "#;
        for line in input.trim().split('\n') {
            println!("parse {line}");
            serde_json::from_str::<Event>(line).unwrap();
        }
    }
}
