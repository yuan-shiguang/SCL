//! Building the JVM command line and supervising the game process.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

use crate::auth::Account;
use crate::download::TaskEvent;
use crate::error::{Error, Result};
use crate::instance::Instance;
use crate::meta::{rules_allow, Features, ResolvedVersion};
use crate::paths::LauncherPaths;
use crate::{LAUNCHER_NAME, LAUNCHER_VERSION};

pub struct LaunchOptions<'a> {
    pub paths: &'a LauncherPaths,
    pub instance: &'a Instance,
    pub version: &'a ResolvedVersion,
    pub account: &'a Account,
    pub java: &'a Path,
    pub memory_mb: u32,
    pub extra_jvm_args: Vec<String>,
    pub extra_game_args: Vec<String>,
}

pub fn classpath_separator() -> &'static str {
    if cfg!(windows) {
        ";"
    } else {
        ":"
    }
}

fn substitute(text: &str, vars: &HashMap<&str, String>) -> String {
    let mut out = text.to_string();
    for (key, value) in vars {
        out = out.replace(&format!("${{{key}}}"), value);
    }
    out
}

fn collect_arguments(
    values: &[serde_json::Value],
    features: &Features,
    vars: &HashMap<&str, String>,
) -> Vec<String> {
    let os = crate::meta::current_os();
    let mut out = Vec::new();
    for value in values {
        match value {
            serde_json::Value::String(s) => out.push(substitute(s, vars)),
            serde_json::Value::Object(map) => {
                let rules: Vec<crate::meta::Rule> = map
                    .get("rules")
                    .and_then(|r| serde_json::from_value(r.clone()).ok())
                    .unwrap_or_default();
                if !rules_allow(&rules, os, features) {
                    continue;
                }
                match map.get("value") {
                    Some(serde_json::Value::String(s)) => out.push(substitute(s, vars)),
                    Some(serde_json::Value::Array(items)) => {
                        for item in items {
                            if let Some(s) = item.as_str() {
                                out.push(substitute(s, vars));
                            }
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
    out
}

pub fn natives_dir(paths: &LauncherPaths, version_id: &str) -> PathBuf {
    paths.version_dir(version_id).join("natives")
}

fn log_config_path(paths: &LauncherPaths, version: &ResolvedVersion) -> Option<PathBuf> {
    let client = version.logging.as_ref()?.client.as_ref()?;
    let file = client.file.as_ref()?;
    let name = file.path.rsplit('/').next().unwrap_or("log_config");
    Some(paths.assets_dir().join("log_configs").join(name))
}

/// Builds the full argument vector (without the executable).
pub fn build_arguments(opts: &LaunchOptions<'_>) -> Result<Vec<String>> {
    let game_dir = opts.instance.game_dir(opts.paths);
    std::fs::create_dir_all(&game_dir)?;
    let natives = natives_dir(opts.paths, &opts.version.id);
    std::fs::create_dir_all(&natives)?;

    // Classpath: every allowed library plus the client jar.
    let features = Features {
        has_custom_resolution: opts.instance.width > 0 && opts.instance.height > 0,
        ..Default::default()
    };
    let mut classpath_entries: Vec<String> = Vec::new();
    for library in &opts.version.libraries {
        if !library.is_allowed(&features) {
            continue;
        }
        if library.natives.is_some() {
            continue;
        }
        let path = opts.paths.libraries_dir().join(library.relative_path());
        if path.is_file() {
            classpath_entries.push(path.to_string_lossy().to_string());
        }
    }
    let client_jar = opts.paths.version_jar(&opts.version.id);
    if client_jar.is_file() {
        classpath_entries.push(client_jar.to_string_lossy().to_string());
    }
    let classpath = classpath_entries.join(classpath_separator());

    let mut vars: HashMap<&str, String> = HashMap::new();
    vars.insert("auth_player_name", opts.account.name.clone());
    vars.insert("version_name", opts.version.id.clone());
    vars.insert("game_directory", game_dir.to_string_lossy().to_string());
    vars.insert(
        "assets_root",
        opts.paths.assets_dir().to_string_lossy().to_string(),
    );
    vars.insert(
        "assets_index_name",
        opts.version
            .asset_index
            .as_ref()
            .map(|a| a.id.clone())
            .or_else(|| opts.version.assets.clone())
            .unwrap_or_else(|| "legacy".to_string()),
    );
    vars.insert("auth_uuid", opts.account.uuid_plain());
    vars.insert("auth_access_token", opts.account.access_token.clone());
    vars.insert("auth_session", opts.account.access_token.clone());
    vars.insert("clientid", String::new());
    vars.insert("auth_xuid", opts.account.xuid.clone());
    vars.insert(
        "user_type",
        if opts.account.is_microsoft() {
            "msa".to_string()
        } else {
            "legacy".to_string()
        },
    );
    vars.insert(
        "version_type",
        opts.version.version_type.clone(),
    );
    vars.insert("user_properties", "{}".to_string());
    vars.insert("natives_directory", natives.to_string_lossy().to_string());
    vars.insert("classpath", classpath.clone());
    vars.insert("classpath_separator", classpath_separator().to_string());
    vars.insert(
        "library_directory",
        opts.paths.libraries_dir().to_string_lossy().to_string(),
    );
    vars.insert(
        "game_assets",
        opts.paths
            .assets_dir()
            .join("virtual")
            .join("legacy")
            .to_string_lossy()
            .to_string(),
    );
    vars.insert("launcher_name", LAUNCHER_NAME.to_string());
    vars.insert("launcher_version", LAUNCHER_VERSION.to_string());
    vars.insert("resolution_width", opts.instance.width.to_string());
    vars.insert("resolution_height", opts.instance.height.to_string());

    // --- JVM arguments -----------------------------------------------------
    let mut args: Vec<String> = Vec::new();
    let memory = opts.memory_mb.max(512);
    args.push(format!("-Xmx{memory}M"));
    args.push(format!("-Xms{}M", (memory / 2).max(256)));
    args.push("-XX:+UseG1GC".to_string());
    // 注意：`G1NewSizePercent` 在 Java 8/21 上都是 experimental，
    // 不加 `-XX:+UnlockExperimentalVMOptions` 会让 JVM 直接拒绝启动，
    // 所以这里只用稳定的调优参数。
    args.push("-XX:MaxGCPauseMillis=50".to_string());
    args.push("-XX:+ParallelRefProcEnabled".to_string());
    args.push("-Dfile.encoding=UTF-8".to_string());
    args.push(format!("-Dminecraft.launcher.brand={LAUNCHER_NAME}"));
    args.push(format!("-Dminecraft.launcher.version={LAUNCHER_VERSION}"));
    #[cfg(target_os = "macos")]
    args.push("-XstartOnFirstThread".to_string());

    if let Some(log) = log_config_path(opts.paths, opts.version) {
        if log.is_file() {
            let mut log_vars: HashMap<&str, String> = HashMap::new();
            log_vars.insert("path", log.to_string_lossy().to_string());
            let argument = opts
                .version
                .logging
                .as_ref()
                .and_then(|l| l.client.as_ref())
                .map(|c| c.argument.clone())
                .unwrap_or_default();
            if !argument.is_empty() {
                args.push(substitute(&argument, &log_vars));
            }
        }
    }

    if opts.version.arguments.jvm.is_empty() {
        args.push(format!(
            "-Djava.library.path={}",
            natives.to_string_lossy()
        ));
        args.push("-cp".to_string());
        args.push(classpath.clone());
    } else {
        args.extend(collect_arguments(
            &opts.version.arguments.jvm,
            &features,
            &vars,
        ));
    }

    for extra in &opts.extra_jvm_args {
        if !extra.trim().is_empty() {
            args.push(extra.trim().to_string());
        }
    }

    args.push(opts.version.main_class.clone());

    // --- game arguments ----------------------------------------------------
    if opts.version.arguments.game.is_empty() {
        if let Some(legacy) = &opts.version.minecraft_arguments {
            for token in legacy.split_whitespace() {
                args.push(substitute(token, &vars));
            }
        }
    } else {
        args.extend(collect_arguments(
            &opts.version.arguments.game,
            &features,
            &vars,
        ));
    }

    for extra in &opts.extra_game_args {
        if !extra.trim().is_empty() {
            args.push(extra.trim().to_string());
        }
    }

    Ok(args)
}

pub struct GameProcess {
    pub child: Child,
    pub log_path: PathBuf,
    pub command_line: String,
}

/// stdout 和 stderr 是不同类型，包一层好统一处理。
enum Pipe {
    Out(std::process::ChildStdout),
    Err(std::process::ChildStderr),
}

fn hide_console(command: &mut Command) -> &mut Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
}

/// Spawns the game. `tx` receives stdout/stderr lines and the exit code.
pub fn launch(opts: &LaunchOptions<'_>, tx: Sender<TaskEvent>) -> Result<GameProcess> {
    let args = build_arguments(opts)?;
    let mut command = Command::new(opts.java);
    command
        .args(&args)
        .current_dir(opts.instance.game_dir(opts.paths))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    hide_console(&mut command);

    let command_line = format!(
        "{} {}",
        opts.java.to_string_lossy(),
        args.join(" ")
    );

    let mut child = command
        .spawn()
        .map_err(|e| Error::Launch(format!("无法启动 Java ({}): {e}", opts.java.display())))?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    std::fs::create_dir_all(opts.paths.logs_dir())?;
    let log_path = opts
        .paths
        .logs_dir()
        .join(format!("{}-latest.log", opts.instance.id));
    let log_file = std::fs::File::create(&log_path).ok();
    let log_file = Arc::new(Mutex::new(log_file));

    for pipe in [stdout.map(Pipe::Out), stderr.map(Pipe::Err)].into_iter().flatten() {
        let tx = tx.clone();
        let log_file = Arc::clone(&log_file);
        std::thread::spawn(move || {
            let reader: Box<dyn BufRead + Send> = match pipe {
                Pipe::Out(p) => Box::new(BufReader::new(p)),
                Pipe::Err(p) => Box::new(BufReader::new(p)),
            };
            for line in reader.lines().map_while(std::result::Result::ok) {
                if let Ok(mut guard) = log_file.lock() {
                    if let Some(file) = guard.as_mut() {
                        let _ = writeln!(file, "{line}");
                    }
                }
                let _ = tx.send(TaskEvent::Log(line));
            }
        });
    }

    Ok(GameProcess {
        child,
        log_path,
        command_line,
    })
}

impl GameProcess {
    /// Waits for the process in the background, reporting the exit code.
    pub fn watch(mut self, tx: Sender<TaskEvent>) -> std::thread::JoinHandle<i32> {
        std::thread::spawn(move || {
            let code = match self.child.wait() {
                Ok(status) => status.code().unwrap_or(-1),
                Err(_) => -1,
            };
            let _ = tx.send(TaskEvent::Log(format!(
                "游戏进程已退出 (exit code {code})"
            )));
            code
        })
    }
}

/// A shell-ish preview of the command, for the "查看启动命令" dialog.
pub fn preview_command(opts: &LaunchOptions<'_>) -> Result<String> {
    let args = build_arguments(opts)?;
    let mut out = String::new();
    out.push_str(&opts.java.to_string_lossy());
    for arg in args {
        out.push('\n');
        if arg.contains(' ') {
            out.push('"');
            out.push_str(&arg);
            out.push('"');
        } else {
            out.push_str(&arg);
        }
    }
    Ok(out)
}
