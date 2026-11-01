//! Java runtime discovery and provisioning.
//!
//! Discovery walks the usual per-platform install locations; when nothing
//! suitable is around, [`install_major`] fetches a Temurin JRE from Adoptium
//! into the launcher's own `java/` directory so the user never has to touch a
//! JDK by hand.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;

use serde::Deserialize;

use crate::download::TaskEvent;
use crate::error::{Error, Result};
use crate::net;
use crate::paths::LauncherPaths;

#[derive(Debug, Clone)]
pub struct JavaRuntime {
    pub path: PathBuf,
    pub major: u32,
    pub version: String,
    pub vendor: String,
}

pub fn java_bin_name() -> &'static str {
    if cfg!(windows) {
        "java.exe"
    } else {
        "java"
    }
}

#[cfg(windows)]
fn hidden(command: &mut Command) -> &mut Command {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NO_WINDOW)
}

#[cfg(not(windows))]
fn hidden(command: &mut Command) -> &mut Command {
    command
}

/// Runs `<java> -version` and parses the reported version.
pub fn probe(path: &Path) -> Option<JavaRuntime> {
    if !path.is_file() {
        return None;
    }
    let mut command = Command::new(path);
    command.arg("-version").stdout(Stdio::piped()).stderr(Stdio::piped());
    let output = hidden(&mut command).output().ok()?;
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    parse_version_output(path, &text)
}

pub fn parse_version_output(path: &Path, text: &str) -> Option<JavaRuntime> {
    let first_line = text
        .lines()
        .find(|l| l.contains("version"))?;
    let version = first_line
        .split('"')
        .nth(1)
        .unwrap_or("")
        .to_string();
    if version.is_empty() {
        return None;
    }
    let major = parse_major(&version)?;
    let vendor = if text.contains("Temurin") || text.contains("Adoptium") {
        "Temurin".to_string()
    } else if text.contains("GraalVM") {
        "GraalVM".to_string()
    } else if text.contains("OpenJ9") {
        "OpenJ9".to_string()
    } else if text.contains("Microsoft") {
        "Microsoft".to_string()
    } else if text.contains("Zulu") {
        "Zulu".to_string()
    } else if text.contains("Oracle") || text.contains("HotSpot") {
        "HotSpot".to_string()
    } else {
        "OpenJDK".to_string()
    };
    Some(JavaRuntime {
        path: path.to_path_buf(),
        major,
        version,
        vendor,
    })
}

fn parse_major(version: &str) -> Option<u32> {
    let mut parts = version.split(['.', '_', '-']);
    let first: u32 = parts.next()?.parse().ok()?;
    if first == 1 {
        // 1.8.0_381 -> 8
        parts.next()?.parse().ok()
    } else {
        Some(first)
    }
}

fn candidates() -> Vec<PathBuf> {
    let bin = java_bin_name();
    let mut out: Vec<PathBuf> = Vec::new();

    if let Ok(home) = std::env::var("JAVA_HOME") {
        if !home.trim().is_empty() {
            out.push(PathBuf::from(home).join("bin").join(bin));
        }
    }

    // Managed runtimes installed by the launcher itself.
    if let Some(data) = dirs::data_dir() {
        let managed = data.join("StellaeCraft").join("java");
        if let Ok(entries) = std::fs::read_dir(&managed) {
            for entry in entries.flatten() {
                out.push(entry.path().join("bin").join(bin));
                out.push(entry.path().join("Contents").join("Home").join("bin").join(bin));
            }
        }
    }

    #[cfg(windows)]
    {
        // 注册表里登记的 JDK / JRE（Oracle、Adoptium、Microsoft 都会写）。
        for home in registry_homes() {
            out.push(home.join("bin").join(bin));
        }

        let mut roots: Vec<PathBuf> = vec![
            PathBuf::from(r"C:\Program Files\Java"),
            PathBuf::from(r"C:\Program Files\Eclipse Adoptium"),
            PathBuf::from(r"C:\Program Files\Eclipse Foundation"),
            PathBuf::from(r"C:\Program Files\AdoptOpenJDK"),
            PathBuf::from(r"C:\Program Files\Microsoft"),
            PathBuf::from(r"C:\Program Files\Semeru"),
            PathBuf::from(r"C:\Program Files\Temurin"),
            PathBuf::from(r"C:\Program Files\Zulu"),
            PathBuf::from(r"C:\Program Files\BellSoft"),
            PathBuf::from(r"C:\Program Files\Amazon Corretto"),
            PathBuf::from(r"C:\Program Files\Android\Android Studio\jbr"),
            PathBuf::from(r"C:\Program Files\Common Files\Oracle\Java\javapath"),
            PathBuf::from(r"C:\Program Files (x86)\Java"),
            PathBuf::from(r"C:\Program Files (x86)\Eclipse Adoptium"),
        ];
        for var in ["LOCALAPPDATA", "APPDATA", "USERPROFILE", "ProgramData"] {
            if let Ok(value) = std::env::var(var) {
                let base = PathBuf::from(value);
                roots.push(base.join(".jdks")); // IntelliJ 下载的 JDK
                roots.push(base.join("Programs").join("Eclipse Adoptium"));
                roots.push(base.join(".minecraft").join("runtime")); // 官方启动器自带
                roots.push(base.join(".hmcl").join("java"));
            }
        }
        for root in &roots {
            push_children(root, bin, &mut out);
            // `.minecraft/runtime/jre-legacy/windows-x64/...` 这类还要再深一层。
            push_grandchildren(root, bin, &mut out);
        }

        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            let local = PathBuf::from(local);
            push_children(&local.join("Programs"), bin, &mut out);
            // Minecraft's own bundled runtimes.
            let packages = local.join("Packages");
            if let Ok(entries) = std::fs::read_dir(&packages) {
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.contains("MinecraftUWP") || name.contains("Microsoft.4297127D64EC6") {
                        let base = entry.path().join("LocalCache").join("Local").join("runtime");
                        push_children(&base, bin, &mut out);
                    }
                }
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        push_children(
            Path::new("/Library/Java/JavaVirtualMachines"),
            bin,
            &mut out,
        );
        if let Some(home) = dirs::home_dir() {
            push_children(
                &home.join("Library/Java/JavaVirtualMachines"),
                bin,
                &mut out,
            );
        }
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        push_children(Path::new("/usr/lib/jvm"), bin, &mut out);
        out.push(PathBuf::from("/usr/bin").join(bin));
    }

    // PATH 上的 java：必须先解析成绝对路径，光有 "java" 这个名字
    // `is_file()` 一定是 false，这正是之前「装了 Java 却扫不到」的原因。
    out.extend(path_java_binaries(bin));

    // Whatever is on PATH (fallback, in case `where`/`which` is unavailable).
    out.push(PathBuf::from(bin));
    out
}

/// 用 `where` / `which` 把 PATH 上的 java 解析成绝对路径。
fn path_java_binaries(bin: &str) -> Vec<PathBuf> {
    let mut command = if cfg!(windows) {
        Command::new("where")
    } else {
        Command::new("which")
    };
    command
        .arg(bin)
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let Ok(output) = hidden(&mut command).output() else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|line| PathBuf::from(line.trim()))
        .filter(|path| path.is_file())
        .collect()
}

/// 注册表里登记的 Java 安装目录（`JavaHome`）。
#[cfg(windows)]
fn registry_homes() -> Vec<PathBuf> {
    const KEYS: &[&str] = &[
        r"HKLM\SOFTWARE\JavaSoft\JDK",
        r"HKLM\SOFTWARE\JavaSoft\JRE",
        r"HKLM\SOFTWARE\JavaSoft\Java Development Kit",
        r"HKLM\SOFTWARE\JavaSoft\Java Runtime Environment",
        r"HKLM\SOFTWARE\Eclipse Adoptium\JDK",
        r"HKLM\SOFTWARE\Eclipse Adoptium\JRE",
        r"HKLM\SOFTWARE\Microsoft\JDK",
        r"HKLM\SOFTWARE\WOW6432Node\JavaSoft\JDK",
        r"HKLM\SOFTWARE\WOW6432Node\JavaSoft\JRE",
    ];
    let mut out = Vec::new();
    for key in KEYS {
        let mut command = Command::new("reg");
        command
            .args(["query", key, "/s", "/v", "JavaHome"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let Ok(output) = hidden(&mut command).output() else {
            continue;
        };
        let text = String::from_utf8_lossy(&output.stdout);
        for line in text.lines() {
            let line = line.trim_start();
            let Some(rest) = line.strip_prefix("JavaHome") else {
                continue;
            };
            let rest = rest.trim_start();
            let Some(rest) = rest.strip_prefix("REG_SZ") else {
                continue;
            };
            let path = rest.trim();
            if !path.is_empty() {
                out.push(PathBuf::from(path));
            }
        }
    }
    out
}

/// 再往下探一层，覆盖 `.minecraft/runtime/jre-legacy/windows-x64/...` 这种结构。
#[cfg(windows)]
fn push_grandchildren(root: &Path, bin: &str, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        if !entry.path().is_dir() {
            continue;
        }
        let Ok(inner) = std::fs::read_dir(entry.path()) else {
            continue;
        };
        for child in inner.flatten() {
            let path = child.path();
            out.push(path.join("bin").join(bin));
            out.push(path.join("Contents").join("Home").join("bin").join(bin));
            out.push(path.join("jre").join("bin").join(bin));
        }
    }
}

#[cfg(windows)]
fn push_children(root: &Path, bin: &str, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        out.push(path.join("bin").join(bin));
        out.push(path.join("Contents").join("Home").join("bin").join(bin));
        out.push(path.join("jre").join("bin").join(bin));
    }
}

#[cfg(not(windows))]
fn push_children(root: &Path, bin: &str, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        out.push(path.join("bin").join(bin));
        out.push(path.join("Contents").join("Home").join("bin").join(bin));
    }
}

/// All usable runtimes, deduplicated and sorted by major version (newest first).
pub fn find_all() -> Vec<JavaRuntime> {
    let mut found: Vec<JavaRuntime> = Vec::new();
    let mut seen: Vec<PathBuf> = Vec::new();
    for candidate in candidates() {
        let key = candidate.canonicalize().unwrap_or_else(|_| candidate.clone());
        if seen.iter().any(|p| p == &key) {
            continue;
        }
        if let Some(runtime) = probe(&candidate) {
            seen.push(key);
            found.push(runtime);
        }
    }
    found.sort_by(|a, b| b.major.cmp(&a.major));
    found
}

/// Picks the runtime that satisfies `required` (or the newest available).
pub fn find_for(required: u32) -> Option<JavaRuntime> {
    let all = find_all();
    all.iter()
        .find(|r| r.major == required)
        .or_else(|| all.iter().find(|r| r.major >= required))
        .or_else(|| all.first())
        .cloned()
}

/// Like [`find_for`], but never falls back to a runtime that is too old.
///
/// A launcher that happily starts 1.21 with Java 8 only produces a wall of
/// `UnsupportedClassVersionError`, so "no match" is the honest answer here.
pub fn find_compatible(required: u32) -> Option<JavaRuntime> {
    let all = find_all();
    if let Some(exact) = all.iter().find(|r| r.major == required) {
        return Some(exact.clone());
    }
    // 没有正好对上的，就取「比要求新的里面最旧的那个」：
    // 1.17 用 21 比用 25 稳，更别说 1.16 配 25 直接炸。
    all.iter()
        .filter(|r| r.major > required)
        .min_by_key(|r| r.major)
        .cloned()
}

// ---------------------------------------------------------------- provisioning

#[derive(Debug, Deserialize)]
struct AdoptiumAsset {
    binary: AdoptiumBinary,
}

#[derive(Debug, Deserialize)]
struct AdoptiumBinary {
    package: AdoptiumPackage,
}

#[derive(Debug, Deserialize)]
struct AdoptiumPackage {
    #[serde(default)]
    checksum: String,
    link: String,
    name: String,
    #[serde(default)]
    #[allow(dead_code)]
    size: u64,
}

fn adoptium_os() -> &'static str {
    if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "macos") {
        "mac"
    } else {
        "linux"
    }
}

fn adoptium_arch() -> &'static str {
    match std::env::consts::ARCH {
        "aarch64" => "aarch64",
        _ => "x64",
    }
}

/// Downloads and unpacks a Temurin JRE for `major` into `<data>/java`.
///
/// Returns the freshly probed runtime so the caller can start using it right
/// away. Existing directories are reused, so calling this twice is cheap.
pub fn install_major(
    major: u32,
    paths: &LauncherPaths,
    sender: &Sender<TaskEvent>,
    cancel: &AtomicBool,
) -> Result<JavaRuntime> {
    let dir = paths.java_dir();
    std::fs::create_dir_all(&dir)?;

    // 已经下载过就直接复用。
    if let Some(found) = scan_dir(&dir).into_iter().find(|r| r.major == major) {
        let _ = sender.send(TaskEvent::Log(format!(
            "Java {major} already available: {}",
            found.path.display()
        )));
        return Ok(found);
    }

    let url = format!(
        "https://api.adoptium.net/v3/assets/latest/{major}/hotspot\
         ?architecture={}&image_type=jre&os={}&vendor=eclipse",
        adoptium_arch(),
        adoptium_os()
    );
    let _ = sender.send(TaskEvent::Stage(format!("Fetching Java {major}")));
    let assets: Vec<AdoptiumAsset> = net::get_json(&url)?;
    let package = assets
        .into_iter()
        .next()
        .ok_or_else(|| Error::Other(format!("Adoptium 上没有 Java {major} 的 JRE 包")))?
        .binary
        .package;

    let archive = dir.join(&package.name);
    let _ = sender.send(TaskEvent::Stage(format!("Downloading Java {major}")));
    let mut last_emit = 0u64;
    net::download_to(&package.link, &archive, |chunk| {
        last_emit += chunk;
        if last_emit >= 2 * 1024 * 1024 {
            last_emit = 0;
            let _ = sender.send(TaskEvent::Progress {
                done: 0,
                total: 1,
                bytes: chunk,
                current: package.name.clone(),
            });
        }
    })?;

    if cancel.load(Ordering::Relaxed) {
        let _ = std::fs::remove_file(&archive);
        return Err(Error::Other("已取消".into()));
    }

    if !package.checksum.is_empty() {
        let actual = sha256_of_file(&archive)?;
        if !actual.eq_ignore_ascii_case(&package.checksum) {
            let _ = std::fs::remove_file(&archive);
            return Err(Error::Other("Java 运行时校验失败，请重试".into()));
        }
    }

    let _ = sender.send(TaskEvent::Stage(format!("Unpacking Java {major}")));
    extract(&archive, &dir)?;
    let _ = std::fs::remove_file(&archive);

    let runtime = scan_dir(&dir)
        .into_iter()
        .find(|r| r.major == major)
        .ok_or_else(|| Error::Other(format!("Java {major} 解压后没有找到可执行文件")))?;
    let _ = sender.send(TaskEvent::Log(format!(
        "Java {major} ready: {}",
        runtime.path.display()
    )));
    Ok(runtime)
}

/// Makes sure some runtime that can run `required` exists, downloading one when
/// nothing on the machine fits.
pub fn ensure(
    required: u32,
    paths: &LauncherPaths,
    sender: &Sender<TaskEvent>,
    cancel: &AtomicBool,
) -> Result<JavaRuntime> {
    if let Some(found) = find_compatible(required) {
        return Ok(found);
    }
    install_major(required, paths, sender, cancel)
}

/// Probes every runtime directly under `dir` (one level deep).
fn scan_dir(dir: &Path) -> Vec<JavaRuntime> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let bin = java_bin_name();
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let base = entry.path();
        if !base.is_dir() {
            continue;
        }
        for candidate in [
            base.join("bin").join(bin),
            base.join("Contents").join("Home").join("bin").join(bin),
            base.join("jre").join("bin").join(bin),
        ] {
            if let Some(runtime) = probe(&candidate) {
                out.push(runtime);
                break;
            }
        }
    }
    out
}

#[cfg(windows)]
fn extract(archive: &Path, dest: &Path) -> Result<()> {
    let file = std::fs::File::open(archive)?;
    let mut zip = zip::ZipArchive::new(file)?;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        let Some(relative) = entry.enclosed_name() else {
            continue;
        };
        let target = dest.join(relative);
        if entry.is_dir() {
            std::fs::create_dir_all(&target)?;
            continue;
        }
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut out = std::fs::File::create(&target)?;
        std::io::copy(&mut entry, &mut out)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Some(mode) = entry.unix_mode() {
                let _ = std::fs::set_permissions(
                    &target,
                    std::fs::Permissions::from_mode(mode),
                );
            }
        }
    }
    Ok(())
}

#[cfg(not(windows))]
fn extract(archive: &Path, dest: &Path) -> Result<()> {
    // tar.gz：交给系统自带的 tar，避免再拉一个解压依赖。
    let status = Command::new("tar")
        .arg("-xzf")
        .arg(archive)
        .arg("-C")
        .arg(dest)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    if !status.success() {
        return Err(Error::Other("解压 Java 运行时失败".into()));
    }
    Ok(())
}

fn sha256_of_file(path: &Path) -> Result<String> {
    use sha2::{Digest, Sha256};
    use std::io::Read;

    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_modern_and_legacy_versions() {
        assert_eq!(parse_major("17.0.8"), Some(17));
        assert_eq!(parse_major("1.8.0_381"), Some(8));
        assert_eq!(parse_major("21"), Some(21));
    }

    #[test]
    fn parses_version_line() {
        let runtime = parse_version_output(
            Path::new("/usr/bin/java"),
            "openjdk version \"17.0.9\" 2023-10-17\nOpenJDK Runtime Environment Temurin-17.0.9+9",
        )
        .unwrap();
        assert_eq!(runtime.major, 17);
        assert_eq!(runtime.vendor, "Temurin");
    }
}
