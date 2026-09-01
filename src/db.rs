use std::path::{Path, PathBuf};

#[derive(Default, serde::Serialize, serde::Deserialize)]
pub struct DB {
    #[serde(skip)]
    pub project_path: PathBuf,
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
        *self = DB {
            project_path: std::mem::take(&mut self.project_path),
            ..toml::from_slice(&buf)?
        };
        Ok(())
    }

    pub fn write(&self) -> anyhow::Result<()> {
        assert!(!self.project_path.as_os_str().is_empty());
        let path = self.meta();
        std::fs::write(&path, toml::to_string(self)?)?;
        Ok(())
    }

    pub fn exe_path(&self) -> PathBuf {
        self.project_path.join(&self.exe)
    }
}
