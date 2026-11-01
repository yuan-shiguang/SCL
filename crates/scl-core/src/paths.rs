use std::path::{Path, PathBuf};

/// Resolves every on-disk location the launcher uses.
///
/// Layout (Windows example, `%APPDATA%\StellaeCraft`):
/// ```text
/// StellaeCraft/
///   launcher.json          launcher settings + accounts
///   instances.json         instance registry
///   minecraft/             shared game data (assets, libraries, versions)
///   instances/<id>/        per-instance game directory
///   java/                  managed runtimes (optional)
///   logs/
/// ```
#[derive(Debug, Clone)]
pub struct LauncherPaths {
    root: PathBuf,
}

impl LauncherPaths {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Default data directory for the current platform.
    pub fn discover() -> Self {
        let base = dirs::data_dir()
            .or_else(dirs::home_dir)
            .unwrap_or_else(|| PathBuf::from("."));
        Self::new(base.join("StellaeCraft"))
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn ensure(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.root)?;
        std::fs::create_dir_all(self.minecraft_dir())?;
        std::fs::create_dir_all(self.instances_dir())?;
        std::fs::create_dir_all(self.logs_dir())?;
        Ok(())
    }

    pub fn minecraft_dir(&self) -> PathBuf {
        self.root.join("minecraft")
    }

    pub fn versions_dir(&self) -> PathBuf {
        self.minecraft_dir().join("versions")
    }

    pub fn libraries_dir(&self) -> PathBuf {
        self.minecraft_dir().join("libraries")
    }

    pub fn assets_dir(&self) -> PathBuf {
        self.minecraft_dir().join("assets")
    }

    pub fn indexes_dir(&self) -> PathBuf {
        self.assets_dir().join("indexes")
    }

    pub fn objects_dir(&self) -> PathBuf {
        self.assets_dir().join("objects")
    }

    pub fn instances_dir(&self) -> PathBuf {
        self.root.join("instances")
    }

    pub fn java_dir(&self) -> PathBuf {
        self.root.join("java")
    }

    pub fn logs_dir(&self) -> PathBuf {
        self.root.join("logs")
    }

    pub fn settings_file(&self) -> PathBuf {
        self.root.join("launcher.json")
    }

    pub fn instances_file(&self) -> PathBuf {
        self.root.join("instances.json")
    }

    pub fn instance_dir(&self, id: &str) -> PathBuf {
        self.instances_dir().join(id)
    }

    /// Version folder inside `minecraft/versions`, keyed by version id.
    pub fn version_dir(&self, version: &str) -> PathBuf {
        self.versions_dir().join(version)
    }

    pub fn version_json(&self, version: &str) -> PathBuf {
        self.version_dir(version).join(format!("{version}.json"))
    }

    pub fn version_jar(&self, version: &str) -> PathBuf {
        self.version_dir(version).join(format!("{version}.jar"))
    }
}
