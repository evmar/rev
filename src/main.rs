mod ai;
mod dis;

#[derive(argh::FromArgs)]
#[argh(subcommand)]
enum Mode {
    Dis(dis::Args),
    AI(ai::Args),
}

/// wip
#[derive(argh::FromArgs)]
struct Args {
    #[argh(subcommand)]
    mode: Mode,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = argh::from_env::<Args>();
    match args.mode {
        Mode::Dis(args) => {
            dis::load(args);
        }
        Mode::AI(_ai) => {
            ai::call().await?;
        }
    }
    Ok(())
}
