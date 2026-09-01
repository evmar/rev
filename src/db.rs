use std::path::PathBuf;

use crate::load::EXE;

#[derive(Default)]
pub struct DB {
    pub project_path: PathBuf,
    pub exe: EXE,
}

impl DB {
    pub fn load(project_path: PathBuf) -> anyhow::Result<Self> {
        let buf = std::fs::read(project_path.join("db.toml"))?;
        let db = DB {
            project_path,
            exe: toml::from_slice(&buf)?,
        };
        Ok(db)
    }

    pub fn write(&self) -> anyhow::Result<()> {
        assert!(!self.project_path.as_os_str().is_empty());
        std::fs::write(self.meta(), toml::to_string(&self.exe)?)?;
        Ok(())
    }

    pub fn project_path(&self, filename: &str) -> PathBuf {
        self.project_path.join(filename)
    }

    pub fn meta(&self) -> PathBuf {
        self.project_path("db.toml")
    }
}
