use std::{
    collections::{BTreeMap, HashSet},
    io::Read,
    path::{Path, PathBuf},
};

use runtime::SegOfs;

use crate::{
    api,
    function::Function,
    load::{EXE, load_exe},
    memory::Memory,
    xref,
};

#[derive(Default)]
pub struct DB {
    pub project_path: PathBuf,
    pub mem: Vec<u8>,
    pub exe: EXE,
    pub functions: BTreeMap<SegOfs, Function>,
    pub memory: Memory,
}

impl DB {
    pub fn load(project_path: PathBuf) -> anyhow::Result<Self> {
        let exe: EXE = toml::from_slice(&std::fs::read(project_path.join("db.toml"))?)?;
        let memory: Memory = toml::from_slice(&std::fs::read(project_path.join("memory.toml"))?)?;

        let mut db = DB {
            project_path,
            exe,
            memory,
            ..Default::default()
        };
        load_exe(&mut db);

        let fn_dir = db.project_path.join("fn");
        for entry in std::fs::read_dir(fn_dir)? {
            let entry = entry?;
            let path = entry.path();
            let buf = std::fs::read(&path)?;
            let func = match Function::deserialize(&db.mem, std::str::from_utf8(&buf)?) {
                Ok(func) => func,
                Err(err) => anyhow::bail!("{}: {}", path.display(), err),
            };
            db.functions.insert(func.ip, func);
        }

        xref::update_all_xrefs(&mut db);

        Ok(db)
    }

    pub fn write(&self) -> anyhow::Result<()> {
        assert!(!self.project_path.as_os_str().is_empty());
        let path = self.meta();
        write_if_changed(&path, toml::to_string(&self.exe)?.as_bytes())?;

        write_if_changed(
            &self.project_path.join("memory.toml"),
            toml::to_string(&self.memory)?.as_bytes(),
        )?;

        let fn_dir = self.project_path("fn");
        std::fs::create_dir_all(&fn_dir)?;
        let mut names = HashSet::new();
        for func in self.functions.values() {
            let name = match &func.name {
                Some(name) => format!("{name}.toml"),
                None => format!("{:04x}_{:04x}.toml", func.ip.seg, func.ip.ofs),
            };
            let path = fn_dir.join(&name);
            let mut contents = String::new();
            func.serialize(&mut contents)?;
            write_if_changed(&path, contents.as_bytes())?;
            names.insert(name);
        }

        for entry in std::fs::read_dir(&fn_dir).unwrap() {
            let entry = entry.unwrap();
            let name = entry.file_name().to_str().unwrap().to_owned();
            if !names.contains(&name) {
                std::fs::remove_file(entry.path())?;
                println!("removed {}", entry.path().display());
            }
        }

        Ok(())
    }

    pub fn project_path(&self, filename: &str) -> PathBuf {
        self.project_path.join(filename)
    }

    pub fn meta(&self) -> PathBuf {
        self.project_path("db.toml")
    }
}

fn write_if_changed(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    match std::fs::read(path) {
        Ok(current) if current == contents => return Ok(()),
        Ok(_) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => return Err(err),
    }
    std::fs::write(path, contents)?;
    println!("wrote {}", path.display());
    Ok(())
}

/// database interaction
#[derive(argh::FromArgs)]
#[argh(subcommand, name = "db")]
pub struct Args {
    #[argh(subcommand)]
    cmd: Command,
}

#[derive(argh::FromArgs)]
#[argh(subcommand)]
enum Command {
    Func(FuncArgs),
}

pub fn run(db: &mut DB, args: Args) -> anyhow::Result<()> {
    match args.cmd {
        Command::Func(args) => run_func(db, args),
    }
}

/// functions
#[derive(argh::FromArgs)]
#[argh(subcommand, name = "func")]
pub struct FuncArgs {
    #[argh(subcommand)]
    cmd: FuncCmd,
}

#[derive(argh::FromArgs)]
#[argh(subcommand)]
enum FuncCmd {
    Read(FuncReadArgs),
    Write(FuncWriteArgs),
}

pub fn run_func(db: &mut DB, FuncArgs { cmd }: FuncArgs) -> anyhow::Result<()> {
    match cmd {
        FuncCmd::Read(args) => run_func_read(db, args),
        FuncCmd::Write(args) => run_func_write(db, args),
    }
}

/// read function
#[derive(argh::FromArgs)]
#[argh(subcommand, name = "read")]
pub struct FuncReadArgs {
    #[argh(positional)]
    addr: String,
}

pub fn run_func_read(db: &mut DB, FuncReadArgs { addr }: FuncReadArgs) -> anyhow::Result<()> {
    let addr = SegOfs::parse(&addr).map_err(|_| anyhow::anyhow!("bad address {:?}", addr))?;
    let func = db
        .functions
        .get(&addr)
        .ok_or_else(|| anyhow::anyhow!("unknown function {addr}"))?;
    let mut buf = String::new();
    func.serialize(&mut buf)?;
    print!("{}", buf);
    Ok(())
}

/// write function
#[derive(argh::FromArgs)]
#[argh(subcommand, name = "write")]
pub struct FuncWriteArgs {
    #[argh(positional)]
    addr: String,
}

pub fn run_func_write(db: &mut DB, FuncWriteArgs { addr }: FuncWriteArgs) -> anyhow::Result<()> {
    if addr == "schema" {
        let schema = api::FunctionUpdate::schema();
        println!("{}", serde_json::to_string(&schema).unwrap());
        return Ok(());
    }

    let addr = SegOfs::parse(&addr).map_err(|_| anyhow::anyhow!("bad address {:?}", addr))?;
    let func = db
        .functions
        .get_mut(&addr)
        .ok_or_else(|| anyhow::anyhow!("unknown function {addr}"))?;

    let mut buf = String::new();
    std::io::stdin().lock().read_to_string(&mut buf)?;
    let update: api::FunctionUpdate = serde_json::from_str(&buf)?;
    update.merge_into(func)?;
    db.write()?;

    Ok(())
}
