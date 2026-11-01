//! 正版皮肤头像。
//!
//! 从 Mojang 会话服务拿到皮肤贴图，裁出 8×8 的头部放大成 64×64 存到
//! `<数据目录>/cache/skins/<uuid>.png`，界面直接 `img(path)` 显示。

use std::path::PathBuf;

use base64::Engine;
use serde::Deserialize;

use crate::auth::Account;
use crate::error::{Error, Result};
use crate::net;
use crate::paths::LauncherPaths;

#[derive(Debug, Deserialize)]
struct Profile {
    #[serde(default)]
    properties: Vec<Property>,
}

#[derive(Debug, Deserialize)]
struct Property {
    name: String,
    value: String,
}

#[derive(Debug, Deserialize)]
struct Textures {
    #[serde(default)]
    textures: TextureMap,
}

#[derive(Debug, Default, Deserialize)]
struct TextureMap {
    #[serde(default, rename = "SKIN")]
    skin: Option<Texture>,
}

#[derive(Debug, Deserialize)]
struct Texture {
    url: String,
}

/// 返回本地头像 PNG 的路径；已经缓存过就直接返回。
pub fn head_icon(paths: &LauncherPaths, account: &Account) -> Result<PathBuf> {
    if !account.is_microsoft() {
        return Err(Error::Other("离线账户没有皮肤".into()));
    }
    let dir = paths.root().join("cache").join("skins");
    std::fs::create_dir_all(&dir)?;
    let target = dir.join(format!("{}.png", account.uuid));
    if target.is_file() {
        return Ok(target);
    }

    let url = format!(
        "https://sessionserver.mojang.com/session/minecraft/profile/{}",
        account.uuid_plain()
    );
    let profile: Profile = net::get_json(&url)?;
    let encoded = profile
        .properties
        .iter()
        .find(|p| p.name == "textures")
        .map(|p| p.value.clone())
        .ok_or_else(|| Error::Other("账号没有皮肤数据".into()))?;

    let decoded = base64::engine::general_purpose::STANDARD
        .decode(encoded.as_bytes())
        .map_err(|e| Error::Other(format!("皮肤数据解码失败: {e}")))?;
    let textures: Textures = serde_json::from_slice(&decoded)?;
    let skin_url = textures
        .textures
        .skin
        .ok_or_else(|| Error::Other("账号没有皮肤".into()))?
        .url;
    // Mojang 返回的是 http://textures.minecraft.net，走 https 更稳。
    let skin_url = skin_url.replace("http://", "https://");

    let bytes = net::get_bytes_with_progress(&skin_url, |_| {})?;
    let image = image::load_from_memory(&bytes)
        .map_err(|e| Error::Other(format!("皮肤图片解析失败: {e}")))?;
    let rgba = image.to_rgba8();
    // 64×64 皮肤的正面头部在 (8, 8)，帽子/眼镜那层在 (40, 8)。
    if rgba.width() < 16 || rgba.height() < 16 {
        return Err(Error::Other("皮肤尺寸异常".into()));
    }
    let mut head = image::imageops::crop_imm(&rgba, 8, 8, 8, 8).to_image();

    // 外层（帽子层）叠在里层上面，这样戴帽子、戴眼镜的皮肤也能正确显示。
    if rgba.width() >= 48 && rgba.height() >= 16 {
        let hat = image::imageops::crop_imm(&rgba, 40, 8, 8, 8).to_image();
        for (x, y, pixel) in hat.enumerate_pixels() {
            let alpha = pixel[3] as f32 / 255.0;
            if alpha <= 0.0 {
                continue;
            }
            let base = head.get_pixel_mut(x, y);
            for channel in 0..3 {
                base[channel] = (pixel[channel] as f32 * alpha
                    + base[channel] as f32 * (1.0 - alpha))
                    .round()
                    .clamp(0.0, 255.0) as u8;
            }
            base[3] = 255;
        }
    }

    let scaled = image::imageops::resize(
        &head,
        64,
        64,
        image::imageops::FilterType::Nearest,
    );
    scaled
        .save(&target)
        .map_err(|e| Error::Other(format!("头像写入失败: {e}")))?;
    Ok(target)
}
