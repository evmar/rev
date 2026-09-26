//mod label_memory;
mod open_router;
mod tool;

use crate::db::DB;

pub async fn run(db: &mut DB, prompt: String) -> anyhow::Result<()> {
    let client = open_router::client()?;
    let mut agent = open_router::AgentLoop::new(db, &client, prompt);
    agent.run().await?;
    Ok(())
}
