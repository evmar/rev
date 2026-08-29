use std::path::{Path, PathBuf};

#[derive(serde::Serialize, serde::Deserialize)]
pub struct DB {
    #[serde(skip)]
    pub project_path: PathBuf,

    pub exe: String,
}

impl DB {
    fn path(project_path: &Path) -> PathBuf {
        Path::new(project_path).join("db.toml")
    }

    pub fn load(project_path: PathBuf) -> anyhow::Result<Self> {
        let buf = std::fs::read(Self::path(&project_path))?;
        let mut db: DB = toml::from_slice(&buf)?;
        db.project_path = project_path;
        Ok(db)
    }

    pub fn write(&self) -> anyhow::Result<PathBuf> {
        let path = Self::path(&self.project_path);
        std::fs::write(&path, toml::to_string(self)?)?;
        Ok(path)
    }

    pub fn exe_path(&self) -> PathBuf {
        self.project_path.join(&self.exe)
    }
}
