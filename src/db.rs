use std::path::PathBuf;

use crate::{dis::Function, load::EXE};

#[derive(Default)]
pub struct DB {
    pub project_path: PathBuf,
    pub exe: EXE,
    pub functions: Vec<Function>,
}

impl DB {
    pub fn load(project_path: PathBuf) -> anyhow::Result<Self> {
        let exe: EXE = toml::from_slice(&std::fs::read(project_path.join("db.toml"))?)?;

        let functions = vec![];
        // for _addr in exe.functions.iter() {}

        let db = DB {
            project_path,
            exe,
            functions,
        };
        Ok(db)
    }

    pub fn write(&self) -> anyhow::Result<()> {
        assert!(!self.project_path.as_os_str().is_empty());
        let path = self.meta();
        std::fs::write(&path, toml::to_string(&self.exe)?)?;
        println!("wrote {}", path.display());

        let fn_dir = self.project_path("fn");
        std::fs::create_dir_all(&fn_dir)?;
        for func in self.functions.iter() {
            let name = format!("{:04x}_{:04x}.toml", func.ip.seg, func.ip.ofs);
            let path = fn_dir.join(name);
            let mut f = std::fs::File::create(&path)?;
            func.serialize(&mut f)?;
            drop(f);
            println!("wrote {}", path.display());
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
