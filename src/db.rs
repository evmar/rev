use std::{collections::BTreeMap, path::PathBuf};

use runtime::SegOfs;

use crate::{
    function::Function,
    load::{EXE, load_exe},
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
            let func = Function::deserialize(
                &db.mem,
                std::str::from_utf8(&std::fs::read(entry.path())?)?,
            )?;
            db.functions.insert(func.ip, func);
        }

        Ok(db)
    }

    pub fn write(&self) -> anyhow::Result<()> {
        assert!(!self.project_path.as_os_str().is_empty());
        let path = self.meta();
        std::fs::write(&path, toml::to_string(&self.exe)?)?;
        println!("wrote {}", path.display());

        let fn_dir = self.project_path("fn");
        std::fs::create_dir_all(&fn_dir)?;
        for func in self.functions.values() {
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
