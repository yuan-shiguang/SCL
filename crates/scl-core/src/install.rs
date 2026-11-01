//! Installing a version: metadata, client jar, libraries, natives and assets.

use std::collections::HashSet;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::Sender;
use std::sync::Arc;

use crate::download::{download_all, write_file, DownloadItem, TaskEvent};
use crate::error::{Error, Result};
use crate::meta::{
    AssetIndex, AssetObject, ResolvedVersion, VersionDetails, VersionManifest, RESOURCES_URL,
    VERSION_MANIFEST_URL,
};
use crate::net;
use crate::paths::LauncherPaths;

pub fn fetch_manifest() -> Result<VersionManifest> {
    net::get_json(VERSION_MANIFEST_URL)
}

/// Downloads (or reads) a single version JSON and resolves its parents.
pub fn resolve_version(paths: &LauncherPaths, version_id: &str) -> Result<ResolvedVersion> {
    resolve_version_depth(paths, version_id, 0)
}

fn resolve_version_depth(
    paths: &LauncherPaths,
    version_id: &str,
    depth: usize,
) -> Result<ResolvedVersion> {
    if depth > 6 {
        return Err(Error::Other(format!(
            "版本 {version_id} 的 inheritsFrom 层级过深"
        )));
    }
    let details = load_or_fetch_version_json(paths, version_id)?;
    let mut resolved = ResolvedVersion::from_details(details);
    if let Some(parent_id) = resolved.inherits_from.clone() {
        let parent = resolve_version_depth(paths, &parent_id, depth + 1)?;
        // Re-apply the child's own fields on top of the parent.
        let child = load_or_fetch_version_json(paths, version_id)?;
        resolved = parent.merge(child);
    }
    Ok(resolved)
}

pub fn load_or_fetch_version_json(
    paths: &LauncherPaths,
    version_id: &str,
) -> Result<VersionDetails> {
    let local = paths.version_json(version_id);
    if local.is_file() {
        if let Ok(text) = std::fs::read_to_string(&local) {
            if let Ok(details) = serde_json::from_str::<VersionDetails>(&text) {
                return Ok(details);
            }
        }
    }
    let manifest = fetch_manifest()?;
    let entry = manifest
        .versions
        .iter()
        .find(|v| v.id == version_id)
        .ok_or_else(|| Error::Other(format!("版本清单中找不到 {version_id}")))?;
    let text = net::get_text(&entry.url)?;
    write_file(&local, text.as_bytes())?;
    Ok(serde_json::from_str(&text)?)
}

/// Full install pipeline. Emits [`TaskEvent`]s while it works.
pub fn install_version(
    paths: &LauncherPaths,
    version_id: &str,
    threads: usize,
    tx: Sender<TaskEvent>,
    cancel: Arc<AtomicBool>,
) -> Result<ResolvedVersion> {
    paths.ensure()?;

    let _ = tx.send(TaskEvent::Stage("正在获取版本信息".into()));
    let resolved = resolve_version(paths, version_id)?;

    // --- client jar + libraries -------------------------------------------
    let _ = tx.send(TaskEvent::Stage("正在下载游戏核心与运行库".into()));
    let mut items: Vec<DownloadItem> = Vec::new();

    if let Some(downloads) = &resolved.downloads {
        if let Some(client) = &downloads.client {
            if !client.url.is_empty() {
                items.push(
                    DownloadItem::new(client.url.clone(), paths.version_jar(version_id))
                        .with_sha1(client.sha1.clone())
                        .with_size(client.size),
                );
            }
        }
    }

    for (artifact, relative) in resolved.artifacts() {
        let dest = if relative == format!("{version_id}.jar") {
            paths.version_jar(version_id)
        } else {
            paths.libraries_dir().join(&relative)
        };
        items.push(
            DownloadItem::new(artifact.url.clone(), dest)
                .with_sha1(artifact.sha1.clone())
                .with_size(artifact.size),
        );
    }

    // Logging configuration (log4j / logback).
    if let Some(logging) = &resolved.logging {
        if let Some(client) = &logging.client {
            if let Some(file) = &client.file {
                if !file.url.is_empty() {
                    let dest = paths
                        .assets_dir()
                        .join("log_configs")
                        .join(file.path.rsplit('/').next().unwrap_or("log_config"));
                    items.push(
                        DownloadItem::new(file.url.clone(), dest)
                            .with_sha1(file.sha1.clone())
                            .with_size(file.size),
                    );
                }
            }
        }
    }

    let report = download_all(items, threads, tx.clone(), cancel.clone());
    if report.failed > 0 {
        let _ = tx.send(TaskEvent::Failed(format!(
            "{} 个文件下载失败：{}",
            report.failed,
            report.errors.first().cloned().unwrap_or_default()
        )));
        return Err(Error::Other("游戏文件下载失败".into()));
    }

    // --- natives ----------------------------------------------------------
    let _ = tx.send(TaskEvent::Stage("正在解压本地库".into()));
    let natives_dir = paths.version_dir(version_id).join("natives");
    extract_natives(paths, &resolved, &natives_dir, &tx)?;

    // --- assets -----------------------------------------------------------
    if let Some(index_ref) = &resolved.asset_index {
        let _ = tx.send(TaskEvent::Stage("正在获取资源索引".into()));
        let index_path = paths.indexes_dir().join(format!("{}.json", index_ref.id));
        let text = net::get_text(&index_ref.url)?;
        write_file(&index_path, text.as_bytes())?;
        let index: AssetIndex = serde_json::from_str(&text)?;

        let _ = tx.send(TaskEvent::Stage("正在下载游戏资源".into()));
        let mut seen: HashSet<String> = HashSet::new();
        let mut asset_items = Vec::new();
        for object in index.objects.values() {
            let rel = object.relative_path();
            if !seen.insert(rel.clone()) {
                continue;
            }
            asset_items.push(
                DownloadItem::new(
                    format!("{RESOURCES_URL}/{rel}"),
                    paths.objects_dir().join(&rel),
                )
                .with_sha1(Some(object.hash.clone()))
                .with_size(Some(object.size)),
            );
        }
        let report = download_all(asset_items, threads, tx.clone(), cancel.clone());
        if report.failed > 0 {
            let _ = tx.send(TaskEvent::Failed(format!(
                "{} 个资源文件下载失败",
                report.failed
            )));
            return Err(Error::Other("游戏资源下载失败".into()));
        }

        if is_legacy_assets(&index_ref.id) {
            let _ = tx.send(TaskEvent::Stage("正在生成旧版资源目录".into()));
            materialize_legacy_assets(paths, &index_ref.id, &index)?;
        }
    }

    let _ = tx.send(TaskEvent::Done {
        downloaded: 0,
        skipped: 0,
        failed: 0,
    });
    Ok(resolved)
}

fn is_legacy_assets(id: &str) -> bool {
    id == "legacy" || id == "pre-1.6"
}

/// Old versions expect `assets/virtual/<index>/<name>` to exist on disk.
fn materialize_legacy_assets(paths: &LauncherPaths, id: &str, index: &AssetIndex) -> Result<()> {
    let root = paths.assets_dir().join("virtual").join(id);
    for (name, object) in &index.objects {
        let src = paths.objects_dir().join(object.relative_path());
        let dest = root.join(name);
        if dest.is_file() {
            continue;
        }
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let _ = std::fs::copy(&src, &dest);
    }
    Ok(())
}

fn extract_natives(
    paths: &LauncherPaths,
    resolved: &ResolvedVersion,
    natives_dir: &Path,
    tx: &Sender<TaskEvent>,
) -> Result<()> {
    if natives_dir.exists() {
        let _ = std::fs::remove_dir_all(natives_dir);
    }
    std::fs::create_dir_all(natives_dir)?;

    let features = crate::meta::Features::default();
    let mut count = 0usize;
    for library in &resolved.libraries {
        if !library.is_allowed(&features) {
            continue;
        }
        if library.natives.is_none() {
            continue;
        }
        let Some(artifact) = library.native_artifact() else {
            continue;
        };
        let jar = if !artifact.path.is_empty() {
            paths.libraries_dir().join(&artifact.path)
        } else {
            paths.libraries_dir().join(library.relative_path())
        };
        if !jar.is_file() {
            continue;
        }
        let exclude: Vec<String> = library
            .extract
            .as_ref()
            .map(|e| e.exclude.clone())
            .unwrap_or_else(|| vec!["META-INF/".to_string()]);

        let file = std::fs::File::open(&jar)?;
        let mut archive = zip::ZipArchive::new(file)?;
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i)?;
            if entry.is_dir() {
                continue;
            }
            let name = entry.name().to_string();
            if exclude.iter().any(|prefix| name.starts_with(prefix)) {
                continue;
            }
            // Keep only the file name: natives jars are flat archives.
            let Some(file_name) = name.rsplit('/').next() else {
                continue;
            };
            let target = natives_dir.join(file_name);
            let mut out = std::fs::File::create(&target)?;
            let mut buf = Vec::new();
            entry.read_to_end(&mut buf)?;
            out.write_all(&buf)?;
            count += 1;
        }
    }
    let _ = tx.send(TaskEvent::Log(format!("已解压 {count} 个本地库文件")));
    Ok(())
}

/// Reads the asset index from disk if present.
pub fn load_asset_index(paths: &LauncherPaths, id: &str) -> Option<AssetIndex> {
    let path = paths.indexes_dir().join(format!("{id}.json"));
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

pub fn asset_objects(index: &AssetIndex) -> Vec<(&String, &AssetObject)> {
    index.objects.iter().collect()
}
