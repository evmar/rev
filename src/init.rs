use std::path::Path;

/// init
#[derive(argh::FromArgs)]
#[argh(subcommand, name = "init")]
pub struct Args {
    /// exe file name
    #[argh(positional)]
    exe: String,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct Meta {
    exe: String,
}

pub fn init(project: String, args: Args) -> Result<(), Box<dyn std::error::Error>> {
    let meta = Meta { exe: args.exe };
    let path = Path::new(&project).join("meta.toml");
    std::fs::write(&path, toml::to_string(&meta)?)?;
    println!("wrote {}", path.display());
    Ok(())
}
