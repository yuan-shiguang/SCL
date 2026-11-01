//! Launcher-wide settings, accounts and UI preferences.

use serde::{Deserialize, Serialize};

use crate::auth::Account;
use crate::error::Result;
use crate::paths::LauncherPaths;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LauncherSettings {
    #[serde(default)]
    pub accounts: Vec<Account>,
    #[serde(default)]
    pub active_account: Option<String>,
    #[serde(default = "default_memory")]
    pub global_memory_mb: u32,
    #[serde(default)]
    pub java_path: Option<String>,
    /// `dark` or `light`.
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_threads")]
    pub download_threads: usize,
    #[serde(default)]
    pub close_launcher_on_launch: bool,
    #[serde(default = "default_true")]
    pub check_sha1: bool,
    #[serde(default)]
    pub last_version_filter: String,
    /// BCP-47 语言代码，例如 `zh-CN` / `en-US` / `ja-JP`。
    #[serde(default = "default_language")]
    pub language: String,
    /// 下载源：`official` / `bmclapi`，或直接写一个镜像基地址。
    #[serde(default = "default_mirror")]
    pub mirror: String,
    /// HTTP 代理，例如 `http://127.0.0.1:7890`；空表示直连。
    #[serde(default)]
    pub proxy: String,
    /// 当前使用的 `.minecraft` 目录；空表示用启动器自己的目录。
    #[serde(default)]
    pub active_game_dir: String,
    /// 添加过的其它 `.minecraft` 目录。
    #[serde(default)]
    pub game_dirs: Vec<String>,
}

/// BMCLAPI 的基地址，国内下载 Mojang 资源快很多。
pub const BMCLAPI_BASE: &str = "https://bmclapi2.bangbang93.com";

fn default_mirror() -> String {
    "official".to_string()
}

fn default_language() -> String {
    "zh-CN".to_string()
}

fn default_memory() -> u32 {
    4096
}

fn default_threads() -> usize {
    8
}

fn default_true() -> bool {
    true
}

fn default_theme() -> String {
    "dark".to_string()
}

impl Default for LauncherSettings {
    fn default() -> Self {
        Self {
            accounts: Vec::new(),
            active_account: None,
            global_memory_mb: default_memory(),
            java_path: None,
            theme: default_theme(),
            download_threads: default_threads(),
            close_launcher_on_launch: false,
            check_sha1: true,
            last_version_filter: String::new(),
            language: default_language(),
            mirror: default_mirror(),
            proxy: String::new(),
            active_game_dir: String::new(),
            game_dirs: Vec::new(),
        }
    }
}

impl LauncherSettings {
    /// 把 `mirror` 字段翻译成实际的镜像基地址（`None` = 官方源）。
    pub fn mirror_base(&self) -> Option<String> {
        match self.mirror.as_str() {
            "" | "official" => None,
            "bmclapi" => Some(BMCLAPI_BASE.to_string()),
            other => Some(other.to_string()),
        }
    }

    /// 把网络配置推给 `net` 模块，启动和保存设置后都要调用一次。
    pub fn apply_network(&self) {
        crate::net::set_mirror(self.mirror_base());
        crate::net::set_proxy(Some(self.proxy.clone()));
    }
    pub fn load(paths: &LauncherPaths) -> Self {
        std::fs::read_to_string(paths.settings_file())
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, paths: &LauncherPaths) -> Result<()> {
        paths.ensure()?;
        std::fs::write(
            paths.settings_file(),
            serde_json::to_string_pretty(self)?,
        )?;
        Ok(())
    }

    pub fn active_account(&self) -> Option<&Account> {
        match &self.active_account {
            Some(name) => self.accounts.iter().find(|a| &a.name == name),
            None => self.accounts.first(),
        }
    }

    pub fn set_account(&mut self, account: Account) {
        self.active_account = Some(account.name.clone());
        match self.accounts.iter_mut().find(|a| a.name == account.name) {
            Some(existing) => *existing = account,
            None => self.accounts.push(account),
        }
    }

    pub fn remove_account(&mut self, name: &str) {
        self.accounts.retain(|a| a.name != name);
        if self.active_account.as_deref() == Some(name) {
            self.active_account = self.accounts.first().map(|a| a.name.clone());
        }
    }
}
