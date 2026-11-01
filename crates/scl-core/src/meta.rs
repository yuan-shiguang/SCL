//! Mojang piston-meta models: version manifest, version JSON, rules and
//! library coordinate parsing.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

pub const VERSION_MANIFEST_URL: &str =
    "https://launchermeta.mojang.com/mc/game/version_manifest_v2.json";
pub const RESOURCES_URL: &str = "https://resources.download.minecraft.net";
pub const LIBRARIES_URL: &str = "https://libraries.minecraft.net";

pub fn current_os() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "osx"
    } else {
        "linux"
    }
}

pub fn os_display_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "Windows"
    } else if cfg!(target_os = "macos") {
        "macOS"
    } else {
        "Linux"
    }
}

/// `x86_64`, `aarch64`, `x86`, ... as reported by Mojang rules.
pub fn current_arch() -> &'static str {
    match std::env::consts::ARCH {
        "x86_64" => "x86_64",
        "x86" => "x86",
        "aarch64" => "arm64",
        other => other,
    }
}

/// Architecture bits used by native classifiers (`${arch}` -> 32/64).
pub fn arch_bits() -> u8 {
    if cfg!(target_pointer_width = "64") {
        64
    } else {
        32
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VersionManifest {
    pub latest: LatestVersions,
    pub versions: Vec<VersionSummary>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LatestVersions {
    pub release: String,
    pub snapshot: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VersionSummary {
    pub id: String,
    #[serde(rename = "type")]
    pub version_type: String,
    pub url: String,
    #[serde(rename = "releaseTime", default)]
    pub release_time: String,
    #[serde(default)]
    pub sha1: Option<String>,
}

impl VersionSummary {
    pub fn is_release(&self) -> bool {
        self.version_type == "release"
    }

    /// `2023-06-12T14:25:32+00:00` -> `2023-06-12`
    pub fn release_date(&self) -> &str {
        self.release_time.get(0..10).unwrap_or("")
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VersionDetails {
    pub id: String,
    #[serde(rename = "type", default)]
    pub version_type: Option<String>,
    #[serde(rename = "mainClass", default)]
    pub main_class: String,
    #[serde(default)]
    pub arguments: Option<Arguments>,
    #[serde(rename = "minecraftArguments", default)]
    pub minecraft_arguments: Option<String>,
    #[serde(default)]
    pub libraries: Vec<Library>,
    #[serde(rename = "assetIndex", default)]
    pub asset_index: Option<AssetIndexRef>,
    #[serde(default)]
    pub assets: Option<String>,
    #[serde(rename = "javaVersion", default)]
    pub java_version: Option<JavaVersion>,
    #[serde(default)]
    pub downloads: Option<VersionDownloads>,
    #[serde(default)]
    pub logging: Option<LoggingConfig>,
    #[serde(rename = "inheritsFrom", default)]
    pub inherits_from: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Arguments {
    #[serde(default)]
    pub game: Vec<serde_json::Value>,
    #[serde(default)]
    pub jvm: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct JavaVersion {
    #[serde(default)]
    pub component: Option<String>,
    #[serde(rename = "majorVersion", default)]
    pub major_version: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VersionDownloads {
    #[serde(default)]
    pub client: Option<Artifact>,
    #[serde(default)]
    pub server: Option<Artifact>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LoggingConfig {
    #[serde(default)]
    pub client: Option<LoggingClient>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LoggingClient {
    #[serde(default)]
    pub argument: String,
    #[serde(default)]
    pub file: Option<Artifact>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AssetIndexRef {
    pub id: String,
    #[serde(default)]
    pub sha1: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(rename = "totalSize", default)]
    pub total_size: Option<u64>,
    pub url: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AssetIndex {
    #[serde(default)]
    pub objects: HashMap<String, AssetObject>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AssetObject {
    pub hash: String,
    #[serde(default)]
    pub size: u64,
}

impl AssetObject {
    /// `ab/cdabcdef...` layout used under `assets/objects`.
    pub fn relative_path(&self) -> String {
        format!("{}/{}", &self.hash[0..2], self.hash)
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Artifact {
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub sha1: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(default)]
    pub url: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Library {
    pub name: String,
    #[serde(default)]
    pub downloads: Option<LibraryDownloads>,
    #[serde(default)]
    pub natives: Option<HashMap<String, String>>,
    #[serde(default)]
    pub rules: Vec<Rule>,
    #[serde(default)]
    pub extract: Option<ExtractRules>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct LibraryDownloads {
    #[serde(default)]
    pub artifact: Option<Artifact>,
    #[serde(default)]
    pub classifiers: Option<HashMap<String, Artifact>>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ExtractRules {
    #[serde(default)]
    pub exclude: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Rule {
    pub action: String,
    #[serde(default)]
    pub os: Option<OsRule>,
    #[serde(default)]
    pub features: Option<HashMap<String, bool>>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct OsRule {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub arch: Option<String>,
}

impl Library {
    pub fn is_allowed(&self, features: &Features) -> bool {
        rules_allow(&self.rules, current_os(), features)
    }

    /// The native classifier this library needs on the current platform, if any.
    pub fn natives_classifier(&self) -> Option<String> {
        let natives = self.natives.as_ref()?;
        let raw = natives.get(current_os())?;
        if raw.is_empty() {
            return None;
        }
        Some(raw.replace(
            "${arch}",
            if arch_bits() == 64 { "64" } else { "32" },
        ))
    }

    pub fn native_artifact(&self) -> Option<&Artifact> {
        let classifier = self.natives_classifier()?;
        let downloads = self.downloads.as_ref()?;
        downloads
            .classifiers
            .as_ref()
            .and_then(|c| c.get(&classifier))
    }

    pub fn artifact(&self) -> Option<&Artifact> {
        self.downloads.as_ref().and_then(|d| d.artifact.as_ref())
    }

    /// Path inside `libraries/`, falling back to coordinates when the JSON
    /// omits `downloads`.
    pub fn relative_path(&self) -> String {
        if let Some(artifact) = self.artifact() {
            if !artifact.path.is_empty() {
                return artifact.path.clone();
            }
        }
        LibraryName::parse(&self.name)
            .map(|n| n.path())
            .unwrap_or_else(|| self.name.replace(':', "/"))
    }
}

/// Feature flags used while evaluating `rules`.
#[derive(Debug, Clone, Default)]
pub struct Features {
    pub has_custom_resolution: bool,
    pub is_demo_user: bool,
    pub has_quick_plays_support: bool,
    pub is_quick_play_singleplayer: bool,
    pub is_quick_play_multiplayer: bool,
    pub is_quick_play_realms: bool,
}

impl Features {
    pub fn get(&self, key: &str) -> Option<bool> {
        match key {
            "has_custom_resolution" => Some(self.has_custom_resolution),
            "is_demo_user" => Some(self.is_demo_user),
            "has_quick_plays_support" => Some(self.has_quick_plays_support),
            "is_quick_play_singleplayer" => Some(self.is_quick_play_singleplayer),
            "is_quick_play_multiplayer" => Some(self.is_quick_play_multiplayer),
            "is_quick_play_realms" => Some(self.is_quick_play_realms),
            _ => None,
        }
    }
}

pub fn rules_allow(rules: &[Rule], os_name: &str, features: &Features) -> bool {
    if rules.is_empty() {
        return true;
    }
    let mut allowed = false;
    for rule in rules {
        if rule_matches(rule, os_name, features) {
            allowed = rule.action == "allow";
        }
    }
    allowed
}

fn rule_matches(rule: &Rule, os_name: &str, features: &Features) -> bool {
    if let Some(os) = &rule.os {
        if let Some(name) = &os.name {
            if !name.eq_ignore_ascii_case(os_name) {
                return false;
            }
        }
        if let Some(arch) = &os.arch {
            if arch != current_arch() {
                return false;
            }
        }
    }
    if let Some(required) = &rule.features {
        for (key, value) in required {
            if features.get(key).unwrap_or(false) != *value {
                return false;
            }
        }
    }
    true
}

/// Parsed Maven coordinate: `group:artifact:version[:classifier][@ext]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryName {
    pub group: String,
    pub artifact: String,
    pub version: String,
    pub classifier: Option<String>,
    pub ext: String,
}

impl LibraryName {
    pub fn parse(raw: &str) -> Option<Self> {
        let (coords, ext) = match raw.split_once('@') {
            Some((coords, ext)) => (coords, ext.to_string()),
            None => (raw, "jar".to_string()),
        };
        let mut parts = coords.split(':');
        let group = parts.next()?.to_string();
        let artifact = parts.next()?.to_string();
        let version = parts.next()?.to_string();
        let classifier = parts.next().map(|s| s.to_string());
        Some(Self {
            group,
            artifact,
            version,
            classifier,
            ext,
        })
    }

    pub fn jar_file_name(&self) -> String {
        match &self.classifier {
            Some(c) => format!("{}-{}-{}.{}", self.artifact, self.version, c, self.ext),
            None => format!("{}-{}.{}", self.artifact, self.version, self.ext),
        }
    }

    pub fn path(&self) -> String {
        format!(
            "{}/{}/{}/{}",
            self.group.replace('.', "/"),
            self.artifact,
            self.version,
            self.jar_file_name()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_library_names() {
        let name = LibraryName::parse("org.lwjgl:lwjgl:3.3.1:natives-windows").unwrap();
        assert_eq!(name.group, "org.lwjgl");
        assert_eq!(name.jar_file_name(), "lwjgl-3.3.1-natives-windows.jar");
        assert_eq!(
            name.path(),
            "org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1-natives-windows.jar"
        );

        let plain = LibraryName::parse("com.mojang:brigadier:1.0.18").unwrap();
        assert_eq!(plain.jar_file_name(), "brigadier-1.0.18.jar");
    }

    #[test]
    fn rules_default_to_allow() {
        assert!(rules_allow(&[], "windows", &Features::default()));
    }
}

/// A version JSON merged with its `inheritsFrom` ancestors (Fabric, Forge,
/// OptiFine ...). Child values win; library and argument lists are combined.
#[derive(Debug, Clone)]
pub struct ResolvedVersion {
    pub id: String,
    pub version_type: String,
    pub main_class: String,
    pub libraries: Vec<Library>,
    pub arguments: Arguments,
    pub minecraft_arguments: Option<String>,
    pub asset_index: Option<AssetIndexRef>,
    pub assets: Option<String>,
    pub java_version: Option<JavaVersion>,
    pub downloads: Option<VersionDownloads>,
    pub logging: Option<LoggingConfig>,
    pub inherits_from: Option<String>,
}

impl ResolvedVersion {
    pub fn from_details(details: VersionDetails) -> Self {
        Self {
            id: details.id,
            version_type: details.version_type.unwrap_or_else(|| "release".into()),
            main_class: details.main_class,
            libraries: details.libraries,
            arguments: details.arguments.unwrap_or_default(),
            minecraft_arguments: details.minecraft_arguments,
            asset_index: details.asset_index,
            assets: details.assets,
            java_version: details.java_version,
            downloads: details.downloads,
            logging: details.logging,
            inherits_from: details.inherits_from,
        }
    }

    /// Applies `child` on top of `self` (which acts as the parent).
    pub fn merge(&self, child: VersionDetails) -> Self {
        let child_args = child.arguments.clone().unwrap_or_default();

        let mut libraries = child.libraries;
        let mut seen: Vec<String> = libraries
            .iter()
            .map(|l| {
                LibraryName::parse(&l.name)
                    .map(|n| format!("{}:{}", n.group, n.artifact))
                    .unwrap_or_else(|| l.name.clone())
            })
            .collect();
        for parent_lib in &self.libraries {
            let key = LibraryName::parse(&parent_lib.name)
                .map(|n| format!("{}:{}", n.group, n.artifact))
                .unwrap_or_else(|| parent_lib.name.clone());
            if !seen.contains(&key) {
                seen.push(key);
                libraries.push(parent_lib.clone());
            }
        }

        let mut game = self.arguments.game.clone();
        game.extend(child_args.game);
        let mut jvm = self.arguments.jvm.clone();
        jvm.extend(child_args.jvm);

        Self {
            id: child.id,
            version_type: child.version_type.unwrap_or_else(|| self.version_type.clone()),
            main_class: if child.main_class.is_empty() {
                self.main_class.clone()
            } else {
                child.main_class
            },
            libraries,
            arguments: Arguments { game, jvm },
            minecraft_arguments: child
                .minecraft_arguments
                .or_else(|| self.minecraft_arguments.clone()),
            asset_index: child.asset_index.or_else(|| self.asset_index.clone()),
            assets: child.assets.or_else(|| self.assets.clone()),
            java_version: child.java_version.or_else(|| self.java_version.clone()),
            downloads: child.downloads.or_else(|| self.downloads.clone()),
            logging: child.logging.or_else(|| self.logging.clone()),
            inherits_from: child.inherits_from,
        }
    }

    pub fn required_java_major(&self) -> u32 {
        self.java_version
            .as_ref()
            .and_then(|j| j.major_version)
            .unwrap_or(8)
    }

    /// Downloads every artifact this version needs (client jar, libraries).
    pub fn artifacts(&self) -> Vec<(Artifact, String)> {
        let mut out = Vec::new();
        if let Some(downloads) = &self.downloads {
            if let Some(client) = &downloads.client {
                if !client.url.is_empty() {
                    out.push((client.clone(), format!("{}.jar", self.id)));
                }
            }
        }
        let features = Features::default();
        for library in &self.libraries {
            if !library.is_allowed(&features) {
                continue;
            }
            if let Some(artifact) = library.artifact() {
                if !artifact.url.is_empty() {
                    out.push((artifact.clone(), library.relative_path()));
                }
            }
            if let Some(native) = library.native_artifact() {
                if !native.url.is_empty() {
                    out.push((native.clone(), native.path.clone()));
                }
            }
        }
        out
    }
}
