//! Instances: an isolated game directory plus its launch configuration.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::auth::now_secs;
use crate::error::Result;
use crate::paths::LauncherPaths;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Instance {
    pub id: String,
    pub name: String,
    /// Minecraft version id, e.g. `1.21.4`.
    pub version: String,
    #[serde(default)]
    pub loader: Option<String>,
    #[serde(default)]
    pub java_path: Option<String>,
    #[serde(default = "default_memory")]
    pub memory_mb: u32,
    #[serde(default)]
    pub jvm_args: String,
    #[serde(default)]
    pub game_args: String,
    #[serde(default)]
    pub width: u32,
    #[serde(default)]
    pub height: u32,
    #[serde(default)]
    pub created_at: u64,
    #[serde(default)]
    pub last_played: u64,
    #[serde(default)]
    pub play_seconds: u64,
    #[serde(default)]
    pub icon: String,
}

fn default_memory() -> u32 {
    4096
}

impl Instance {
    pub fn new(id: impl Into<String>, name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            version: version.into(),
            loader: None,
            java_path: None,
            memory_mb: default_memory(),
            jvm_args: String::new(),
            game_args: String::new(),
            width: 1280,
            height: 720,
            created_at: now_secs(),
            last_played: 0,
            play_seconds: 0,
            icon: String::new(),
        }
    }

    pub fn game_dir(&self, paths: &LauncherPaths) -> PathBuf {
        paths.instance_dir(&self.id)
    }

    pub fn display_version(&self) -> String {
        match &self.loader {
            Some(loader) if !loader.is_empty() => format!("{} · {}", self.version, loader),
            _ => self.version.clone(),
        }
    }

    pub fn play_time_label(&self) -> String {
        if self.play_seconds == 0 {
            return "从未启动".to_string();
        }
        let hours = self.play_seconds / 3600;
        let minutes = (self.play_seconds % 3600) / 60;
        if hours > 0 {
            format!("{hours} 小时 {minutes} 分")
        } else {
            format!("{minutes} 分钟")
        }
    }

    /// Deterministic accent hue used by the UI for the instance tile.
    pub fn accent_index(&self) -> usize {
        let sum: u32 = self.id.bytes().map(|b| b as u32).sum();
        (sum % 6) as usize
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct InstancesFile {
    #[serde(default)]
    instances: Vec<Instance>,
}

#[derive(Debug, Clone)]
pub struct InstanceStore {
    path: PathBuf,
    instances: Vec<Instance>,
}

impl InstanceStore {
    pub fn load(paths: &LauncherPaths) -> Self {
        let path = paths.instances_file();
        let instances = std::fs::read_to_string(&path)
            .ok()
            .and_then(|text| serde_json::from_str::<InstancesFile>(&text).ok())
            .map(|file| file.instances)
            .unwrap_or_default();
        Self { path, instances }
    }

    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = InstancesFile {
            instances: self.instances.clone(),
        };
        std::fs::write(&self.path, serde_json::to_string_pretty(&file)?)?;
        Ok(())
    }

    pub fn list(&self) -> &[Instance] {
        &self.instances
    }

    pub fn is_empty(&self) -> bool {
        self.instances.is_empty()
    }

    pub fn get(&self, id: &str) -> Option<&Instance> {
        self.instances.iter().find(|i| i.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut Instance> {
        self.instances.iter_mut().find(|i| i.id == id)
    }

    pub fn upsert(&mut self, instance: Instance) {
        match self.get_mut(&instance.id) {
            Some(existing) => *existing = instance,
            None => self.instances.push(instance),
        }
    }

    pub fn remove(&mut self, id: &str) -> Option<Instance> {
        let index = self.instances.iter().position(|i| i.id == id)?;
        Some(self.instances.remove(index))
    }

    /// Generates a unique slug such as `1-21-4` or `1-21-4-2`.
    pub fn unique_id(&self, base: &str) -> String {
        let slug: String = base
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
            .collect();
        let slug = slug.trim_matches('-').to_string();
        let slug = if slug.is_empty() { "instance".to_string() } else { slug };
        if self.get(&slug).is_none() {
            return slug;
        }
        for n in 2..1000 {
            let candidate = format!("{slug}-{n}");
            if self.get(&candidate).is_none() {
                return candidate;
            }
        }
        format!("{slug}-{}", now_secs())
    }
}
