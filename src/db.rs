use std::path::{Path, PathBuf};

#[derive(Default)]
pub struct DB {
    pub project_path: PathBuf,
    pub meta: Meta,
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
pub struct Meta {
    pub exe: String,
}

impl DB {
    pub fn meta(&self) -> PathBuf {
        Path::new(&self.project_path).join("db.toml")
    }

    pub fn new(project_path: PathBuf) -> Self {
        DB {
            project_path,
            ..Default::default()
        }
    }

    pub fn load(&mut self) -> anyhow::Result<()> {
        let buf = std::fs::read(self.meta())?;
        self.meta = toml::from_slice(&buf)?;
        Ok(())
    }

    pub fn write(&self) -> anyhow::Result<()> {
        let path = self.meta();
        std::fs::write(&path, toml::to_string(&self.meta)?)?;
        Ok(())
    }

    pub fn exe_path(&self) -> PathBuf {
        self.project_path.join(&self.meta.exe)
    }
}
