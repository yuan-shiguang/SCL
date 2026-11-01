use std::io::Read;
use std::path::Path;
use std::sync::RwLock;
use std::time::Duration;

use serde::de::DeserializeOwned;

use crate::error::{Error, Result};

pub const USER_AGENT: &str = concat!(
    "StellaeCraftLauncher/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/yuan-shiguang/SCL)"
);

/// 官方源 → BMCLAPI 的路径映射。
///
/// BMCLAPI 对 Mojang 的三类主机是同名直通，只有 assets / maven / 元数据
/// 需要额外前缀，所以这里单独列出后缀。
const MIRROR_ROUTES: &[(&str, &str)] = &[
    ("https://launchermeta.mojang.com", ""),
    ("https://piston-meta.mojang.com", ""),
    ("https://launcher.mojang.com", ""),
    ("https://resources.download.minecraft.net", "/assets"),
    ("https://libraries.minecraft.net", "/maven"),
    ("https://meta.fabricmc.net", "/fabric-meta"),
    ("https://meta.quiltmc.org", "/quilt-meta"),
    ("https://maven.fabricmc.net", "/maven"),
    ("https://maven.quiltmc.org", "/maven"),
    ("https://maven.minecraftforge.net", "/maven"),
    ("https://files.minecraftforge.net", "/forge"),
];

static MIRROR: RwLock<Option<String>> = RwLock::new(None);

/// 设置下载镜像。传 `None` 回到官方源。
///
/// 只影响「从 Mojang / 加载器官方站」下载的地址，GitHub、Adoptium 等
/// 第三方主机保持原样。
pub fn set_mirror(base: Option<String>) {
    if let Ok(mut guard) = MIRROR.write() {
        *guard = base.map(|b| b.trim_end_matches('/').to_string());
    }
}

pub fn mirror() -> Option<String> {
    MIRROR.read().ok().and_then(|g| g.clone())
}

/// 按当前镜像改写下载地址；没有镜像时原样返回。
pub fn rewrite(url: &str) -> String {
    let Some(base) = mirror() else {
        return url.to_string();
    };
    for (from, suffix) in MIRROR_ROUTES {
        if let Some(rest) = url.strip_prefix(from) {
            return format!("{base}{suffix}{rest}");
        }
    }
    url.to_string()
}

fn agent() -> ureq::Agent {
    let mut builder = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(15))
        .timeout(Duration::from_secs(120))
        .user_agent(USER_AGENT);
    if let Some(proxy) = proxy_url() {
        if let Ok(p) = ureq::Proxy::new(&proxy) {
            builder = builder.proxy(p);
        }
    }
    builder.build()
}

/// 用户配置的 HTTP 代理（`http://127.0.0.1:7890` 之类），空表示直连。
static PROXY: RwLock<Option<String>> = RwLock::new(None);

pub fn set_proxy(url: Option<String>) {
    if let Ok(mut guard) = PROXY.write() {
        *guard = url.filter(|u| !u.trim().is_empty());
    }
}

fn proxy_url() -> Option<String> {
    PROXY.read().ok().and_then(|g| g.clone())
}

pub fn get_text(url: &str) -> Result<String> {
    let resp = agent().get(&rewrite(url)).call()?;
    Ok(resp.into_string()?)
}

pub fn get_json<T: DeserializeOwned>(url: &str) -> Result<T> {
    let text = get_text(url)?;
    Ok(serde_json::from_str(&text)?)
}

pub fn post_json<T: DeserializeOwned>(url: &str, body: &serde_json::Value) -> Result<T> {
    let resp = agent()
        .post(url)
        .set("Content-Type", "application/json")
        .set("Accept", "application/json")
        .send_json(body.clone())?;
    let text = resp.into_string()?;
    Ok(serde_json::from_str(&text)?)
}

/// POST with an explicit bearer token (Microsoft / Minecraft services).
pub fn post_json_bearer<T: DeserializeOwned>(
    url: &str,
    body: &serde_json::Value,
    token: &str,
) -> Result<T> {
    let resp = agent()
        .post(url)
        .set("Content-Type", "application/json")
        .set("Accept", "application/json")
        .set("Authorization", &format!("Bearer {token}"))
        .send_json(body.clone())?;
    let text = resp.into_string()?;
    Ok(serde_json::from_str(&text)?)
}

pub fn get_json_bearer<T: DeserializeOwned>(url: &str, token: &str) -> Result<T> {
    let resp = agent()
        .get(url)
        .set("Accept", "application/json")
        .set("Authorization", &format!("Bearer {token}"))
        .call()?;
    let text = resp.into_string()?;
    Ok(serde_json::from_str(&text)?)
}

/// POST form-encoded (OAuth token endpoint).
pub fn post_form<T: DeserializeOwned>(url: &str, form: &[(&str, &str)]) -> Result<T> {
    let resp = agent()
        .post(url)
        .set("Content-Type", "application/x-www-form-urlencoded")
        .set("Accept", "application/json")
        .send_form(form)?;
    let text = resp.into_string()?;
    Ok(serde_json::from_str(&text)?)
}

/// POST JSON but keep the body on HTTP error statuses (OAuth needs it).
pub fn post_json_text(url: &str, body: &serde_json::Value) -> Result<(u16, String)> {
    match agent()
        .post(url)
        .set("Content-Type", "application/json")
        .set("Accept", "application/json")
        .send_json(body.clone())
    {
        Ok(resp) => {
            let status = resp.status();
            Ok((status, resp.into_string().unwrap_or_default()))
        }
        Err(ureq::Error::Status(code, resp)) => Ok((code, resp.into_string().unwrap_or_default())),
        Err(e) => Err(Error::Http(e.to_string())),
    }
}

/// POST form-encoded but keep the body on HTTP error statuses.
pub fn post_form_text(url: &str, form: &[(&str, &str)]) -> Result<(u16, String)> {
    match agent()
        .post(url)
        .set("Content-Type", "application/x-www-form-urlencoded")
        .set("Accept", "application/json")
        .send_form(form)
    {
        Ok(resp) => {
            let status = resp.status();
            Ok((status, resp.into_string().unwrap_or_default()))
        }
        Err(ureq::Error::Status(code, resp)) => Ok((code, resp.into_string().unwrap_or_default())),
        Err(e) => Err(Error::Http(e.to_string())),
    }
}

/// Downloads `url` into memory, reporting total bytes as they arrive.
pub fn get_bytes_with_progress(url: &str, mut on_chunk: impl FnMut(u64)) -> Result<Vec<u8>> {
    let resp = agent().get(&rewrite(url)).call()?;
    let mut reader = resp.into_reader();
    let mut buf = Vec::new();
    let mut chunk = [0u8; 64 * 1024];
    loop {
        let n = reader.read(&mut chunk)?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
        on_chunk(n as u64);
    }
    Ok(buf)
}

/// Streams `url` into `dest`, creating parent directories as needed.
pub fn download_to(url: &str, dest: &Path, mut on_chunk: impl FnMut(u64)) -> Result<u64> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let resp = agent().get(&rewrite(url)).call()?;
    let mut reader = resp.into_reader();
    let tmp = dest.with_extension("part");
    let mut out = std::fs::File::create(&tmp)?;
    let mut total = 0u64;
    let mut chunk = [0u8; 64 * 1024];
    loop {
        let n = reader.read(&mut chunk).map_err(|e| Error::Http(e.to_string()))?;
        if n == 0 {
            break;
        }
        std::io::Write::write_all(&mut out, &chunk[..n])?;
        total += n as u64;
        on_chunk(n as u64);
    }
    drop(out);
    std::fs::rename(&tmp, dest)?;
    Ok(total)
}

pub fn sha1_of_file(path: &Path) -> Result<String> {
    use sha1::{Digest, Sha1};
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha1::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = std::io::Read::read(&mut file, &mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex(&hasher.finalize()))
}

pub fn sha1_of_bytes(bytes: &[u8]) -> String {
    use sha1::{Digest, Sha1};
    let mut hasher = Sha1::new();
    hasher.update(bytes);
    hex(&hasher.finalize())
}

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}
