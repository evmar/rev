mod ai;
mod dis;
mod init;

#[derive(argh::FromArgs)]
#[argh(subcommand)]
enum Mode {
    Init(init::Args),
    Dis(dis::Args),
    AI(ai::Args),
}

/// wip
#[derive(argh::FromArgs)]
struct Args {
    /// project path
    #[argh(option)]
    project: String,

    #[argh(subcommand)]
    mode: Mode,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let Args { project, mode } = argh::from_env::<Args>();
    match mode {
        Mode::Init(args) => init::init(project, args),
        Mode::Dis(args) => {
            dis::load(args);
            Ok(())
        }
        Mode::AI(_ai) => ai::call().await,
    }
}
