use std::{
    collections::{BTreeMap, HashSet},
    path::{Path, PathBuf},
};

use runtime::SegOfs;

use crate::{
    function::Function,
    load::{EXE, load_exe},
    xref,
};

#[derive(Default)]
pub struct DB {
    pub project_path: PathBuf,
    pub mem: Vec<u8>,
    pub exe: EXE,
    pub functions: BTreeMap<SegOfs, Function>,
}

impl DB {
    pub fn load(project_path: PathBuf) -> anyhow::Result<Self> {
        let exe: EXE = toml::from_slice(&std::fs::read(project_path.join("db.toml"))?)?;
        let mut db = DB {
            project_path,
            exe,
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

        xref::update_xrefs(&mut db);

        Ok(db)
    }

    pub fn write(&self) -> anyhow::Result<()> {
        assert!(!self.project_path.as_os_str().is_empty());
        let path = self.meta();
        write_if_changed(&path, toml::to_string(&self.exe)?.as_bytes())?;

        let fn_dir = self.project_path("fn");
        std::fs::create_dir_all(&fn_dir)?;
        let mut names = HashSet::new();
        for func in self.functions.values() {
            let name = match &func.name {
                Some(name) => format!("{name}.toml"),
                None => format!("{:04x}_{:04x}.toml", func.ip.seg, func.ip.ofs),
            };
            let path = fn_dir.join(&name);
            let mut contents = Vec::new();
            func.serialize(&mut contents)?;
            write_if_changed(&path, &contents)?;
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
