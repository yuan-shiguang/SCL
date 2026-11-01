//! 第三方加载器（Fabric / Quilt）安装。
//!
//! 两者都提供「生成一份带 `inheritsFrom` 的版本 JSON」的接口，装好这份 JSON
//! 之后就能复用原版的安装流水线。

use serde::Deserialize;

use crate::error::{Error, Result};
use crate::net;
use crate::paths::LauncherPaths;

const FABRIC_LOADERS: &str = "https://meta.fabricmc.net/v2/versions/loader";
const FABRIC_PROFILE: &str = "https://meta.fabricmc.net/v2/versions/loader";
const QUILT_LOADERS: &str = "https://meta.quiltmc.org/v3/versions/loader";
const QUILT_PROFILE: &str = "https://meta.quiltmc.org/v3/versions/loader";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoaderKind {
    Vanilla,
    Fabric,
    Quilt,
}

impl LoaderKind {
    pub fn all() -> [LoaderKind; 3] {
        [LoaderKind::Vanilla, LoaderKind::Fabric, LoaderKind::Quilt]
    }

    pub fn key(&self) -> &'static str {
        match self {
            LoaderKind::Vanilla => "vanilla",
            LoaderKind::Fabric => "fabric",
            LoaderKind::Quilt => "quilt",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            LoaderKind::Vanilla => "Vanilla",
            LoaderKind::Fabric => "Fabric",
            LoaderKind::Quilt => "Quilt",
        }
    }

    pub fn from_key(key: &str) -> LoaderKind {
        match key {
            "fabric" => LoaderKind::Fabric,
            "quilt" => LoaderKind::Quilt,
            _ => LoaderKind::Vanilla,
        }
    }

    pub fn is_vanilla(&self) -> bool {
        matches!(self, LoaderKind::Vanilla)
    }
}

#[derive(Debug, Clone)]
pub struct LoaderVersion {
    pub version: String,
    pub stable: bool,
}

#[derive(Debug, Deserialize)]
struct FabricEntry {
    loader: FabricLoader,
}

#[derive(Debug, Deserialize)]
struct FabricLoader {
    version: String,
    #[serde(default)]
    stable: bool,
}

#[derive(Debug, Deserialize)]
struct QuiltLoader {
    version: String,
    #[serde(default)]
    stable: bool,
}

/// 列出某个游戏版本可用的加载器版本（新版本在前）。
pub fn fetch_loader_versions(kind: LoaderKind, game_version: &str) -> Result<Vec<LoaderVersion>> {
    match kind {
        LoaderKind::Vanilla => Ok(Vec::new()),
        LoaderKind::Fabric => {
            let url = format!("{FABRIC_LOADERS}/{game_version}");
            let entries: Vec<FabricEntry> = net::get_json(&url)?;
            Ok(entries
                .into_iter()
                .map(|e| LoaderVersion {
                    version: e.loader.version,
                    stable: e.loader.stable,
                })
                .collect())
        }
        LoaderKind::Quilt => {
            let url = format!("{QUILT_LOADERS}/{game_version}");
            let entries: Vec<QuiltLoader> = net::get_json(&url)?;
            Ok(entries
                .into_iter()
                .map(|e| LoaderVersion {
                    version: e.version,
                    stable: e.stable,
                })
                .collect())
        }
    }
}

/// 下载加载器的 profile JSON 并写入 `versions/<id>/<id>.json`，返回版本 id。
pub fn install_loader(
    paths: &LauncherPaths,
    kind: LoaderKind,
    game_version: &str,
    loader_version: &str,
) -> Result<String> {
    if kind.is_vanilla() {
        return Ok(game_version.to_string());
    }
    let url = match kind {
        LoaderKind::Fabric => format!("{FABRIC_PROFILE}/{game_version}/{loader_version}/profile/json"),
        LoaderKind::Quilt => format!("{QUILT_PROFILE}/{game_version}/{loader_version}/profile/json"),
        LoaderKind::Vanilla => unreachable!(),
    };

    let text = net::get_text(&url).map_err(|e| {
        Error::Other(format!(
            "{} {loader_version} 没有适配 {game_version}：{e}",
            kind.display_name()
        ))
    })?;

    // 读取 profile 里的 id；拿不到就按官方命名规则拼一个。
    let value: serde_json::Value = serde_json::from_str(&text)?;
    let id = value
        .get("id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| match kind {
            LoaderKind::Fabric => format!("fabric-loader-{loader_version}-{game_version}"),
            LoaderKind::Quilt => format!("quilt-loader-{loader_version}-{game_version}"),
            LoaderKind::Vanilla => game_version.to_string(),
        });

    let dest = paths.version_json(&id);
    crate::download::write_file(&dest, text.as_bytes())?;
    Ok(id)
}

/// 把加载器版本号拼成界面上的展示名，例如 `Fabric 0.16.9`。
pub fn loader_label(kind: LoaderKind, loader_version: &str) -> String {
    if kind.is_vanilla() {
        "Vanilla".to_string()
    } else {
        format!("{} {}", kind.display_name(), loader_version)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vanilla_returns_game_version() {
        let paths = LauncherPaths::new(std::env::temp_dir().join("scl-loader-test"));
        assert_eq!(
            install_loader(&paths, LoaderKind::Vanilla, "1.21.4", "").unwrap(),
            "1.21.4"
        );
    }

    #[test]
    fn loader_keys_round_trip() {
        for kind in LoaderKind::all() {
            assert_eq!(LoaderKind::from_key(kind.key()), kind);
        }
    }
}
