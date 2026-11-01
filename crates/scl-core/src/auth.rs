//! Accounts: Microsoft (device-code OAuth -> Xbox Live -> XSTS -> Minecraft
//! services) and offline mode.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::net;

/// Stellae Craft Launcher 在 Azure 上注册的应用（已通过 Mojang 白名单审核）。
pub const CLIENT_ID: &str = "3d71cd0f-26f1-4d3a-9889-cdbe20506ace";
/// 应用所在目录（租户）。
pub const TENANT_ID: &str = "993f705b-ab92-4f24-92b7-36415614dc5f";
pub const SCOPE: &str = "XboxLive.signin offline_access";

/// Azure 应用配置。
///
/// `tenant` 默认使用 `common`，这样个人微软账号（Minecraft 玩家基本都是）
/// 才能走通 OAuth；如果应用被限制为「仅此组织目录中的账户」，把
/// `tenant` 改成 [`TENANT_ID`]，或设置环境变量 `SCL_MS_TENANT`。
#[derive(Debug, Clone)]
pub struct MsApp {
    pub client_id: String,
    pub tenant: String,
}

impl Default for MsApp {
    fn default() -> Self {
        Self {
            client_id: CLIENT_ID.to_string(),
            tenant: std::env::var("SCL_MS_TENANT").unwrap_or_else(|_| "common".to_string()),
        }
    }
}

impl MsApp {
    pub fn device_code_url(&self) -> String {
        format!(
            "https://login.microsoftonline.com/{}/oauth2/v2.0/devicecode",
            self.tenant
        )
    }

    pub fn token_url(&self) -> String {
        format!(
            "https://login.microsoftonline.com/{}/oauth2/v2.0/token",
            self.tenant
        )
    }
}

const XBL_URL: &str = "https://user.auth.xboxlive.com/user/authenticate";
const XSTS_URL: &str = "https://xsts.auth.xboxlive.com/xsts/authorize";
const MC_LOGIN_URL: &str = "https://api.minecraftservices.com/authentication/login_with_xbox";
const MC_PROFILE_URL: &str = "https://api.minecraftservices.com/minecraft/profile";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AccountKind {
    Offline,
    Microsoft,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub kind: AccountKind,
    pub name: String,
    pub uuid: String,
    #[serde(default)]
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: String,
    #[serde(default)]
    pub xuid: String,
    /// Unix seconds; 0 means "never expires" (offline accounts).
    #[serde(default)]
    pub expires_at: u64,
}

impl Account {
    pub fn offline(name: &str) -> Self {
        Self {
            kind: AccountKind::Offline,
            name: name.to_string(),
            uuid: offline_uuid(name),
            access_token: "0".to_string(),
            refresh_token: String::new(),
            xuid: String::new(),
            expires_at: 0,
        }
    }

    pub fn is_microsoft(&self) -> bool {
        self.kind == AccountKind::Microsoft
    }

    pub fn token_valid(&self) -> bool {
        if !self.is_microsoft() {
            return true;
        }
        now_secs() + 60 < self.expires_at
    }

    /// Short uuid without dashes, as used by some servers.
    pub fn uuid_plain(&self) -> String {
        self.uuid.replace('-', "")
    }
}

pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Java's `UUID.nameUUIDFromBytes(("OfflinePlayer:" + name).getBytes())`.
pub fn offline_uuid(name: &str) -> String {
    let digest = md5::compute(format!("OfflinePlayer:{name}").as_bytes());
    let mut b = digest.0;
    b[6] = (b[6] & 0x0f) | 0x30;
    b[8] = (b[8] & 0x3f) | 0x80;
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7], b[8], b[9], b[10], b[11], b[12], b[13], b[14],
        b[15]
    )
}

/// Device code returned by the Microsoft identity platform.
#[derive(Debug, Clone)]
pub struct DeviceCode {
    pub user_code: String,
    pub device_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
    pub message: String,
}

#[derive(Debug, Default, Deserialize)]
struct MsResponse {
    #[serde(default)]
    access_token: Option<String>,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    expires_in: Option<u64>,
    #[serde(default)]
    user_code: Option<String>,
    #[serde(default)]
    device_code: Option<String>,
    #[serde(default)]
    verification_uri: Option<String>,
    #[serde(default)]
    interval: Option<u64>,
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    error_description: Option<String>,
    #[serde(default)]
    token_type: Option<String>,
}

#[derive(Debug, Deserialize)]
struct XboxResponse {
    #[serde(rename = "Token")]
    token: String,
    #[serde(rename = "DisplayClaims")]
    display_claims: DisplayClaims,
}

#[derive(Debug, Deserialize)]
struct DisplayClaims {
    xui: Vec<Xui>,
}

#[derive(Debug, Deserialize)]
struct Xui {
    uhs: String,
    #[serde(default)]
    xid: Option<String>,
}

#[derive(Debug, Deserialize)]
struct McLoginResponse {
    #[serde(default)]
    access_token: String,
    #[serde(default)]
    expires_in: u64,
}

#[derive(Debug, Deserialize)]
struct McProfile {
    id: String,
    name: String,
}

fn ms_error(resp: &MsResponse, fallback: &str) -> Error {
    let detail = resp
        .error_description
        .clone()
        .or_else(|| resp.error.clone())
        .unwrap_or_else(|| fallback.to_string());
    Error::Auth(detail)
}

/// Step 1: request a device code. The user must open `verification_uri`.
pub fn request_device_code(app: &MsApp) -> Result<DeviceCode> {
    // Microsoft 的 devicecode 端点只认 form-encoded，发 JSON 会得到
    // AADSTS900144（"body must contain 'client_id'"）。
    let (status, body) = net::post_form_text(
        &app.device_code_url(),
        &[("client_id", app.client_id.as_str()), ("scope", SCOPE)],
    )?;
    let resp: MsResponse = serde_json::from_str(&body)
        .map_err(|e| Error::Auth(format!("设备码响应解析失败 ({status}): {e}")))?;
    if resp.user_code.is_none() {
        return Err(ms_error(&resp, "无法获取设备码"));
    }
    Ok(DeviceCode {
        user_code: resp.user_code.unwrap_or_default(),
        device_code: resp.device_code.unwrap_or_default(),
        verification_uri: resp
            .verification_uri
            .unwrap_or_else(|| "https://microsoft.com/link".to_string()),
        expires_in: resp.expires_in.unwrap_or(900),
        interval: resp.interval.unwrap_or(5).max(1),
        message: resp.message.unwrap_or_default(),
    })
}

/// Polls the token endpoint until the user approves or the code expires.
pub fn poll_device_code(
    app: &MsApp,
    device: &DeviceCode,
    cancel: &AtomicBool,
    mut on_wait: impl FnMut(&str),
) -> Result<MsResponse> {
    let deadline = std::time::Instant::now() + Duration::from_secs(device.expires_in.max(60));
    let mut interval = device.interval.max(1);
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(Error::Auth("已取消登录".into()));
        }
        if std::time::Instant::now() > deadline {
            return Err(Error::Auth("设备码已过期，请重新登录".into()));
        }
        std::thread::sleep(Duration::from_secs(interval));
        if cancel.load(Ordering::Relaxed) {
            return Err(Error::Auth("已取消登录".into()));
        }

        let (_, body) = net::post_form_text(
            &app.token_url(),
            &[
                ("client_id", app.client_id.as_str()),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ("device_code", &device.device_code),
            ],
        )?;
        let resp: MsResponse = serde_json::from_str(&body)
            .map_err(|e| Error::Auth(format!("令牌响应解析失败: {e}")))?;

        if resp.access_token.is_some() {
            return Ok(resp);
        }
        match resp.error.as_deref() {
            Some("authorization_pending") => on_wait("等待你在浏览器中完成授权…"),
            Some("slow_down") => {
                interval += 5;
                on_wait("请求过于频繁，已放慢轮询…");
            }
            Some("authorization_declined") => {
                return Err(Error::Auth("授权被拒绝".into()))
            }
            Some("expired_token") => {
                return Err(Error::Auth("设备码已过期，请重新登录".into()))
            }
            _ => return Err(ms_error(&resp, "登录失败")),
        }
    }
}

fn xbox_authenticate(ms_access_token: &str) -> Result<XboxResponse> {
    let body = serde_json::json!({
        "Properties": {
            "AuthMethod": "RPS",
            "SiteName": "user.auth.xboxlive.com",
            "RpsTicket": format!("d={ms_access_token}")
        },
        "RelyingParty": "http://auth.xboxlive.com",
        "TokenType": "JWT"
    });
    let (status, text) = net::post_json_text(XBL_URL, &body)?;
    if status != 200 {
        return Err(Error::Auth(format!("Xbox Live 认证失败 ({status}): {text}")));
    }
    serde_json::from_str(&text).map_err(|e| Error::Auth(format!("Xbox 响应解析失败: {e}")))
}

fn xsts_authorize(xbl_token: &str) -> Result<XboxResponse> {
    let body = serde_json::json!({
        "Properties": {
            "SandboxId": "RETAIL",
            "UserTokens": [xbl_token]
        },
        "RelyingParty": "rp://api.minecraftservices.com/",
        "TokenType": "JWT"
    });
    let (status, text) = net::post_json_text(XSTS_URL, &body)?;
    if status != 200 {
        let hint = if text.contains("2148916233") {
            "该 Microsoft 账号还没有 Xbox 档案，请先登录 xbox.com 创建"
        } else if text.contains("2148916238") {
            "该账号属于未成年家庭组，需要家长同意"
        } else {
            "XSTS 授权失败"
        };
        return Err(Error::Auth(format!("{hint} ({status})")));
    }
    serde_json::from_str(&text).map_err(|e| Error::Auth(format!("XSTS 响应解析失败: {e}")))
}

fn minecraft_login(xsts: &XboxResponse) -> Result<McLoginResponse> {
    let uhs = xsts
        .display_claims
        .xui
        .first()
        .map(|x| x.uhs.clone())
        .ok_or_else(|| Error::Auth("XSTS 响应缺少 uhs".into()))?;
    let body = serde_json::json!({
        "identityToken": format!("XBL3.0 x={uhs};{}", xsts.token)
    });
    let (status, text) = net::post_json_text(MC_LOGIN_URL, &body)?;
    if status != 200 {
        return Err(Error::Auth(format!(
            "Minecraft 登录失败 ({status})：该账号可能未拥有游戏"
        )));
    }
    serde_json::from_str(&text).map_err(|e| Error::Auth(format!("Minecraft 响应解析失败: {e}")))
}

fn fetch_profile(token: &str) -> Result<McProfile> {
    let profile: McProfile = net::get_json_bearer(MC_PROFILE_URL, token)?;
    Ok(profile)
}

fn finish_login(
    token: &MsResponse,
    xsts: &XboxResponse,
    mc: &McLoginResponse,
    profile: &McProfile,
) -> Account {
    Account {
        kind: AccountKind::Microsoft,
        name: profile.name.clone(),
        uuid: profile.id.clone(),
        access_token: mc.access_token.clone(),
        refresh_token: token.refresh_token.clone().unwrap_or_default(),
        xuid: xsts
            .display_claims
            .xui
            .first()
            .and_then(|x| x.xid.clone())
            .unwrap_or_default(),
        expires_at: now_secs() + mc.expires_in,
    }
}

/// Full interactive login. `on_code` fires as soon as the code is available so
/// the UI can display it while this thread keeps polling.
pub fn microsoft_login(
    app: &MsApp,
    cancel: &AtomicBool,
    mut on_code: impl FnMut(&DeviceCode),
    mut on_wait: impl FnMut(&str),
) -> Result<Account> {
    let device = request_device_code(app)?;
    on_code(&device);
    let token = poll_device_code(app, &device, cancel, &mut on_wait)?;
    let ms_token = token
        .access_token
        .clone()
        .ok_or_else(|| Error::Auth("缺少 access_token".into()))?;

    on_wait("正在通过 Xbox Live 验证…");
    let xbl = xbox_authenticate(&ms_token)?;
    on_wait("正在获取 XSTS 授权…");
    let xsts = xsts_authorize(&xbl.token)?;
    on_wait("正在登录 Minecraft 服务…");
    let mc = minecraft_login(&xsts)?;
    on_wait("正在读取玩家档案…");
    let profile = fetch_profile(&mc.access_token)?;
    Ok(finish_login(&token, &xsts, &mc, &profile))
}

/// Refreshes an expired Microsoft session using the stored refresh token.
pub fn refresh(app: &MsApp, account: &Account) -> Result<Account> {
    if account.refresh_token.is_empty() {
        return Err(Error::Auth("登录已过期，请重新登录".into()));
    }
    let (_, body) = net::post_form_text(
        &app.token_url(),
        &[
            ("client_id", app.client_id.as_str()),
            ("grant_type", "refresh_token"),
            ("refresh_token", &account.refresh_token),
            ("scope", SCOPE),
        ],
    )?;
    let token: MsResponse = serde_json::from_str(&body)
        .map_err(|e| Error::Auth(format!("刷新令牌解析失败: {e}")))?;
    let ms_token = token
        .access_token
        .clone()
        .ok_or_else(|| ms_error(&token, "刷新登录失败，请重新登录"))?;
    let xbl = xbox_authenticate(&ms_token)?;
    let xsts = xsts_authorize(&xbl.token)?;
    let mc = minecraft_login(&xsts)?;
    let profile = fetch_profile(&mc.access_token)?;
    Ok(finish_login(&token, &xsts, &mc, &profile))
}

/// Ensures the account has a usable game token, refreshing when needed.
pub fn ensure_valid(app: &MsApp, account: &Account) -> Result<Account> {
    if !account.is_microsoft() || account.token_valid() {
        return Ok(account.clone());
    }
    refresh(app, account)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_uuid_matches_java() {
        assert_eq!(
            offline_uuid("Notch"),
            "b50ad385-829d-3141-a216-7e7d7539ba7f"
        );
    }
}
