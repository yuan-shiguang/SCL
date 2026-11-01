//! 应用外壳：状态、导航、任务编排与渲染。

use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::time::Duration;

use futures::StreamExt;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{ActiveTheme, Disableable, Selectable, Theme, ThemeMode};
use gpui_kit::*;
use scl_core::auth::{self, Account};
use scl_core::config::LauncherSettings;
use scl_core::download::TaskEvent;
use scl_core::instance::{Instance, InstanceStore};
use scl_core::java::{self, JavaRuntime};
use scl_core::launch::{self, LaunchOptions};
use scl_core::loader::{self, LoaderKind, LoaderVersion};
use scl_core::meta::VersionManifest;
use scl_core::{install, LauncherPaths};

use crate::i18n::{self, Lang};
use crate::task::{bridge, cancel_token};
use crate::ui;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Home,
    Versions,
    Instance,
    Tasks,
    Settings,
}

impl Page {
    pub fn key(&self) -> &'static str {
        match self {
            Page::Home => "nav.home",
            Page::Versions => "nav.versions",
            Page::Instance => "instance.title",
            Page::Tasks => "nav.tasks",
            Page::Settings => "nav.settings",
        }
    }

    pub fn glyph(&self) -> &'static str {
        match self {
            Page::Home => "◆",
            Page::Versions => "▤",
            Page::Instance => "▩",
            Page::Tasks => "⇣",
            Page::Settings => "⚙",
        }
    }

    /// 侧边栏里出现的页面（实例页从启动台进入）。
    pub fn nav_items() -> [Page; 4] {
        [Page::Home, Page::Versions, Page::Tasks, Page::Settings]
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum VersionFilter {
    Release,
    Snapshot,
    All,
}

impl VersionFilter {
    pub fn key(&self) -> &'static str {
        match self {
            VersionFilter::Release => "versions.release",
            VersionFilter::Snapshot => "versions.snapshot",
            VersionFilter::All => "versions.all",
        }
    }
}

#[derive(Clone)]
pub struct Toast {
    pub message: String,
    pub error: bool,
}

#[derive(Clone, PartialEq)]
pub enum TaskStatus {
    Running,
    Success,
    Failed(String),
    Cancelled,
}

#[derive(Clone, PartialEq)]
pub enum TaskKind {
    Install {
        game: String,
        kind: LoaderKind,
        loader_version: Option<String>,
        instance_id: String,
        launch_after: bool,
    },
    Run {
        instance: String,
    },
    /// 下载托管 Java 运行时。
    Java {
        major: u32,
    },
    Generic,
}

pub struct TaskEntry {
    pub id: u64,
    pub title: String,
    pub subtitle: String,
    pub stage: String,
    pub current_file: String,
    pub done: usize,
    pub total: usize,
    pub bytes: u64,
    pub status: TaskStatus,
    pub logs: Vec<String>,
    pub log_burst: usize,
    pub kind: TaskKind,
    pub cancel: Arc<std::sync::atomic::AtomicBool>,
}

impl TaskEntry {
    pub fn progress(&self) -> f32 {
        if self.total == 0 {
            return 0.0;
        }
        (self.done as f32 / self.total as f32) * 100.0
    }

    pub fn is_running(&self) -> bool {
        matches!(self.status, TaskStatus::Running)
    }
}

#[derive(Clone)]
pub enum LoginView {
    Hidden,
    Device { code: String, uri: String, hint: String },
    Busy(String),
    Failed(String),
}

#[derive(Clone)]
pub struct InstallDialog {
    pub version: String,
    pub kind: LoaderKind,
    pub loader_versions: Vec<LoaderVersion>,
    pub selected: Option<String>,
    pub loading: bool,
    pub error: Option<String>,
}

#[derive(Clone)]
pub struct ModEntry {
    pub file_name: String,
    pub enabled: bool,
    pub size: u64,
}

pub struct LauncherApp {
    pub paths: LauncherPaths,
    pub settings: LauncherSettings,
    pub instances: InstanceStore,
    pub page: Page,

    pub manifest: Option<VersionManifest>,
    pub manifest_error: Option<String>,
    pub manifest_loading: bool,
    pub search: Entity<InputState>,
    pub filter: VersionFilter,
    pub shown: usize,

    pub tasks: Vec<TaskEntry>,
    next_task_id: u64,

    pub javas: Vec<JavaRuntime>,
    pub java_scanning: bool,

    pub selected_instance: Option<String>,
    pub running_instance: Option<String>,

    pub login: LoginView,
    pub offline_name: Entity<InputState>,
    pub toast: Option<Toast>,
    pub command_preview: Option<(String, String)>,
    pub confirm_delete: Option<String>,
    pub install_dialog: Option<InstallDialog>,

    _subscriptions: Vec<Subscription>,
}

impl LauncherApp {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let paths = LauncherPaths::discover();
        let _ = paths.ensure();
        let mut settings = LauncherSettings::load(&paths);
        if settings.language.is_empty() {
            settings.language = Lang::detect().code().to_string();
        }
        let instances = InstanceStore::load(&paths);

        let search = cx.new(|cx| InputState::new(window, cx).placeholder("1.21"));
        let offline_name = cx.new(|cx| InputState::new(window, cx).placeholder("Player"));

        let mut subscriptions = Vec::new();
        subscriptions.push(cx.subscribe_in(
            &search,
            window,
            |this: &mut Self, _state, event: &InputEvent, _window, cx| {
                if matches!(event, InputEvent::Change) {
                    this.shown = 40;
                    cx.notify();
                }
            },
        ));

        let mut app = Self {
            paths,
            settings,
            instances,
            page: Page::Home,
            manifest: None,
            manifest_error: None,
            manifest_loading: false,
            search,
            filter: VersionFilter::Release,
            shown: 40,
            tasks: Vec::new(),
            next_task_id: 1,
            javas: Vec::new(),
            java_scanning: false,
            selected_instance: None,
            running_instance: None,
            login: LoginView::Hidden,
            offline_name,
            toast: None,
            command_preview: None,
            confirm_delete: None,
            install_dialog: None,
            _subscriptions: subscriptions,
        };
        app.selected_instance = app.instances.list().first().map(|i| i.id.clone());
        app.apply_theme(cx);
        app.refresh_manifest(cx);
        app.detect_java(cx);
        app
    }

    /// 把设置里的 `theme` 落到 GPUI Kit 的全局主题上。
    pub fn apply_theme(&self, cx: &mut Context<Self>) {
        let mode = if self.settings.theme == "light" {
            ThemeMode::Light
        } else {
            ThemeMode::Dark
        };
        Theme::change(mode, None, cx);
    }

    pub fn set_theme(&mut self, theme: &str, cx: &mut Context<Self>) {
        self.settings.theme = theme.to_string();
        let _ = self.settings.save(&self.paths);
        self.apply_theme(cx);
        cx.notify();
    }

    // ------------------------------------------------------------------ i18n

    pub fn lang(&self) -> Lang {
        Lang::from_code(&self.settings.language)
    }

    pub fn t(&self, key: &str) -> &'static str {
        i18n::tr(self.lang(), key)
    }

    pub fn tf(&self, key: &str, args: &[(&str, &str)]) -> String {
        i18n::tf(self.lang(), key, args)
    }

    pub fn set_language(&mut self, lang: Lang, cx: &mut Context<Self>) {
        self.settings.language = lang.code().to_string();
        let _ = self.settings.save(&self.paths);
        cx.notify();
    }

    // ----------------------------------------------------------------- tasks

    fn next_id(&mut self) -> u64 {
        let id = self.next_task_id;
        self.next_task_id += 1;
        id
    }

    fn push_task(
        &mut self,
        title: String,
        subtitle: String,
        kind: TaskKind,
        cx: &mut Context<Self>,
    ) -> u64 {
        let id = self.next_id();
        self.tasks.insert(
            0,
            TaskEntry {
                id,
                title,
                subtitle,
                stage: self.t("tasks.stage.preparing").to_string(),
                current_file: String::new(),
                done: 0,
                total: 0,
                bytes: 0,
                status: TaskStatus::Running,
                logs: Vec::new(),
                log_burst: 0,
                kind,
                cancel: cancel_token(),
            },
        );
        self.page = Page::Tasks;
        cx.notify();
        id
    }

    /// 在后台线程跑 `work`，把事件泵回 UI。
    pub fn spawn_engine<F>(&mut self, id: u64, work: F, cx: &mut Context<Self>)
    where
        F: FnOnce(
                Sender<TaskEvent>,
                Arc<std::sync::atomic::AtomicBool>,
            ) -> std::result::Result<(), String>
            + Send
            + 'static,
    {
        let Some(entry) = self.tasks.iter().find(|t| t.id == id) else {
            return;
        };
        let cancel = Arc::clone(&entry.cancel);
        let bridge = bridge();
        let sender = bridge.sender.clone();
        let cancel_for_work = Arc::clone(&cancel);

        std::thread::spawn(move || {
            let result = work(sender.clone(), cancel_for_work);
            match result {
                Ok(()) => {
                    let _ = sender.send(TaskEvent::Log("__task_ok__".to_string()));
                }
                Err(err) => {
                    let _ = sender.send(TaskEvent::Failed(err));
                }
            }
        });

        let mut receiver = bridge.receiver;
        cx.spawn(async move |this, cx| {
            while let Some(event) = receiver.next().await {
                let stop = this
                    .update(cx, |app: &mut Self, cx| app.on_task_event(id, event, cx))
                    .unwrap_or(true);
                if stop {
                    break;
                }
            }
        })
        .detach();
    }

    /// 返回 `true` 表示任务已结束，泵可以停下。
    fn on_task_event(&mut self, id: u64, event: TaskEvent, cx: &mut Context<Self>) -> bool {
        let lang = self.lang();
        let Some(task) = self.tasks.iter_mut().find(|t| t.id == id) else {
            return true;
        };
        match event {
            TaskEvent::Stage(stage) => {
                task.stage = stage;
            }
            TaskEvent::Progress {
                done,
                total,
                bytes,
                current,
            } => {
                task.done = done;
                task.total = total;
                task.bytes = bytes;
                if !current.is_empty() {
                    task.current_file = current;
                }
            }
            TaskEvent::Log(line) => {
                if line == "__task_ok__" {
                    task.status = TaskStatus::Success;
                    task.stage = i18n::tr(lang, "tasks.stage.done").to_string();
                    let kind = task.kind.clone();
                    cx.notify();
                    self.on_task_finished(kind, true, cx);
                    return true;
                }

                // 加载器装好后回传真实版本 id，写回实例。
                if let Some(version_id) = line.strip_prefix("__resolved__") {
                    let version_id = version_id.to_string();
                    if let TaskKind::Install {
                        kind,
                        loader_version,
                        instance_id,
                        ..
                    } = task.kind.clone()
                    {
                        let label = loader::loader_label(
                            kind,
                            loader_version.as_deref().unwrap_or(""),
                        );
                        if let Some(instance) = self.instances.get_mut(&instance_id) {
                            instance.version = version_id;
                            instance.loader = Some(label);
                            let _ = self.instances.save();
                        }
                    }
                    return false;
                }

                if line == "__launched__" {
                    if self.settings.close_launcher_on_launch
                        && matches!(task.kind, TaskKind::Run { .. })
                    {
                        cx.quit();
                    }
                    return false;
                }

                task.logs.push(line);
                if task.logs.len() > 400 {
                    task.logs.remove(0);
                }
                task.log_burst += 1;
                if task.log_burst >= 8 {
                    task.log_burst = 0;
                    cx.notify();
                }
                return false;
            }
            TaskEvent::Done { .. } => {}
            TaskEvent::Failed(message) => {
                task.status = TaskStatus::Failed(message.clone());
                task.stage = message;
                let kind = task.kind.clone();
                cx.notify();
                self.on_task_finished(kind, false, cx);
                return true;
            }
        }
        cx.notify();
        false
    }

    fn on_task_finished(&mut self, kind: TaskKind, success: bool, cx: &mut Context<Self>) {
        match kind {
            TaskKind::Install {
                game,
                instance_id,
                launch_after,
                ..
            } => {
                if success {
                    let msg = self.tf("toast.installed", &[("version", &game)]);
                    self.set_toast(msg, false, cx);
                    if launch_after {
                        self.play(&instance_id, cx);
                    }
                }
            }
            TaskKind::Run { instance } => {
                if self.running_instance.as_deref() == Some(instance.as_str()) {
                    self.running_instance = None;
                }
                if let Some(inst) = self.instances.get_mut(&instance) {
                    inst.last_played = auth::now_secs();
                }
                let _ = self.instances.save();
                if success {
                    let msg = self.t("toast.game_exited").to_string();
                    self.set_toast(msg, false, cx);
                }
            }
            TaskKind::Java { major } => {
                if success {
                    let msg = self.tf("toast.java_ready", &[("major", &major.to_string())]);
                    self.set_toast(msg, false, cx);
                    self.detect_java(cx);
                }
            }
            TaskKind::Generic => {}
        }
        cx.notify();
    }

    pub fn set_toast(&mut self, message: impl Into<String>, error: bool, cx: &mut Context<Self>) {
        self.toast = Some(Toast {
            message: message.into(),
            error,
        });
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_secs(4)).await;
            let _ = this.update(cx, |app: &mut Self, cx| {
                app.toast = None;
                cx.notify();
            });
        })
        .detach();
    }

    // ---------------------------------------------------------------- actions

    pub fn refresh_manifest(&mut self, cx: &mut Context<Self>) {
        if self.manifest_loading {
            return;
        }
        self.manifest_loading = true;
        self.manifest_error = None;
        cx.notify();

        let bridge = bridge();
        let sender = bridge.sender;
        std::thread::spawn(move || match install::fetch_manifest() {
            Ok(manifest) => {
                let _ = sender.send(TaskEvent::Log(format!(
                    "__manifest_ok__{}",
                    serde_json::to_string(&manifest).unwrap_or_default()
                )));
            }
            Err(err) => {
                let _ = sender.send(TaskEvent::Failed(err.to_string()));
            }
        });

        let mut receiver = bridge.receiver;
        cx.spawn(async move |this, cx| {
            while let Some(event) = receiver.next().await {
                let _ = this.update(cx, |app: &mut Self, cx| {
                    app.manifest_loading = false;
                    match event {
                        TaskEvent::Log(line) => {
                            if let Some(json) = line.strip_prefix("__manifest_ok__") {
                                match serde_json::from_str::<VersionManifest>(json) {
                                    Ok(manifest) => {
                                        app.manifest = Some(manifest);
                                        app.manifest_error = None;
                                    }
                                    Err(_) => {
                                        app.manifest_error =
                                            Some(app.t("toast.manifest_failed").to_string());
                                    }
                                }
                            }
                        }
                        TaskEvent::Failed(err) => app.manifest_error = Some(err),
                        _ => {}
                    }
                    cx.notify();
                });
                break;
            }
        })
        .detach();
    }

    pub fn detect_java(&mut self, cx: &mut Context<Self>) {
        if self.java_scanning {
            return;
        }
        self.java_scanning = true;
        cx.notify();
        let bridge = bridge();
        let sender = bridge.sender;
        std::thread::spawn(move || {
            let runtimes = java::find_all();
            let payload: Vec<(String, u32, String, String)> = runtimes
                .iter()
                .map(|r| {
                    (
                        r.path.to_string_lossy().to_string(),
                        r.major,
                        r.version.clone(),
                        r.vendor.clone(),
                    )
                })
                .collect();
            let _ = sender.send(TaskEvent::Log(format!(
                "__java_ok__{}",
                serde_json::to_string(&payload).unwrap_or_default()
            )));
        });

        let mut receiver = bridge.receiver;
        cx.spawn(async move |this, cx| {
            while let Some(event) = receiver.next().await {
                let _ = this.update(cx, |app: &mut Self, cx| {
                    app.java_scanning = false;
                    if let TaskEvent::Log(line) = event {
                        if let Some(json) = line.strip_prefix("__java_ok__") {
                            if let Ok(list) =
                                serde_json::from_str::<Vec<(String, u32, String, String)>>(json)
                            {
                                app.javas = list
                                    .into_iter()
                                    .map(|(path, major, version, vendor)| JavaRuntime {
                                        path: PathBuf::from(path),
                                        major,
                                        version,
                                        vendor,
                                    })
                                    .collect();
                            }
                        }
                    }
                    cx.notify();
                });
                break;
            }
        })
        .detach();
    }

    /// 下载一个由启动器托管的 Temurin JRE（装到 `<data>/java`）。
    pub fn download_java(&mut self, major: u32, cx: &mut Context<Self>) {
        let title = self.tf("tasks.java_title", &[("major", &major.to_string())]);
        let task_id = self.push_task(
            title,
            format!("Temurin {major}"),
            TaskKind::Java { major },
            cx,
        );
        let paths = self.paths.clone();
        self.spawn_engine(
            task_id,
            move |sender, cancel| {
                java::install_major(major, &paths, &sender, &cancel)
                    .map(|_| ())
                    .map_err(|e| e.to_string())
            },
            cx,
        );
    }

    // -------------------------------------------------------- install dialog
    pub fn open_install_dialog(&mut self, version: String, cx: &mut Context<Self>) {
        self.install_dialog = Some(InstallDialog {
            version,
            kind: LoaderKind::Vanilla,
            loader_versions: Vec::new(),
            selected: None,
            loading: false,
            error: None,
        });
        cx.notify();
    }

    pub fn close_install_dialog(&mut self, cx: &mut Context<Self>) {
        self.install_dialog = None;
        cx.notify();
    }

    pub fn choose_loader(&mut self, kind: LoaderKind, cx: &mut Context<Self>) {
        let Some(dialog) = self.install_dialog.clone() else {
            return;
        };
        self.install_dialog = Some(InstallDialog {
            kind,
            loader_versions: Vec::new(),
            selected: None,
            loading: !kind.is_vanilla(),
            error: None,
            ..dialog.clone()
        });
        cx.notify();
        if kind.is_vanilla() {
            return;
        }

        let game = dialog.version.clone();
        let bridge = bridge();
        let sender = bridge.sender;
        std::thread::spawn(move || match loader::fetch_loader_versions(kind, &game) {
            Ok(versions) => {
                let payload: Vec<(String, bool)> =
                    versions.into_iter().map(|v| (v.version, v.stable)).collect();
                let _ = sender.send(TaskEvent::Log(format!(
                    "__loaders__{}",
                    serde_json::to_string(&payload).unwrap_or_default()
                )));
            }
            Err(err) => {
                let _ = sender.send(TaskEvent::Failed(err.to_string()));
            }
        });

        let mut receiver = bridge.receiver;
        cx.spawn(async move |this, cx| {
            while let Some(event) = receiver.next().await {
                let _ = this.update(cx, |app: &mut Self, cx| {
                    if let Some(dialog) = app.install_dialog.as_mut() {
                        dialog.loading = false;
                        match event {
                            TaskEvent::Log(line) => {
                                if let Some(json) = line.strip_prefix("__loaders__") {
                                    if let Ok(list) =
                                        serde_json::from_str::<Vec<(String, bool)>>(json)
                                    {
                                        let versions: Vec<LoaderVersion> = list
                                            .into_iter()
                                            .map(|(version, stable)| LoaderVersion {
                                                version,
                                                stable,
                                            })
                                            .collect();
                                        dialog.selected = versions
                                            .iter()
                                            .find(|v| v.stable)
                                            .or_else(|| versions.first())
                                            .map(|v| v.version.clone());
                                        dialog.loader_versions = versions;
                                    }
                                }
                            }
                            TaskEvent::Failed(err) => dialog.error = Some(err),
                            _ => {}
                        }
                    }
                    cx.notify();
                });
                break;
            }
        })
        .detach();
    }

    pub fn pick_loader_version(&mut self, version: String, cx: &mut Context<Self>) {
        if let Some(dialog) = self.install_dialog.as_mut() {
            dialog.selected = Some(version);
            cx.notify();
        }
    }

    pub fn confirm_install(&mut self, cx: &mut Context<Self>) {
        let Some(dialog) = self.install_dialog.clone() else {
            return;
        };
        if !dialog.kind.is_vanilla() && dialog.selected.is_none() {
            return;
        }
        self.install_dialog = None;

        let game = dialog.version.clone();
        let instance_id = self.instances.unique_id(&game);
        let mut instance = Instance::new(&instance_id, &game, &game);
        instance.memory_mb = self.settings.global_memory_mb;
        if !dialog.kind.is_vanilla() {
            instance.loader = Some(loader::loader_label(
                dialog.kind,
                dialog.selected.as_deref().unwrap_or(""),
            ));
        }
        let game_dir = instance.game_dir(&self.paths);
        let _ = std::fs::create_dir_all(game_dir);
        self.instances.upsert(instance);
        let _ = self.instances.save();
        self.selected_instance = Some(instance_id.clone());

        let subtitle = if dialog.kind.is_vanilla() {
            game.clone()
        } else {
            loader::loader_label(dialog.kind, dialog.selected.as_deref().unwrap_or(""))
        };

        let task_id = self.push_task(
            format!("Install {game}"),
            subtitle,
            TaskKind::Install {
                game: game.clone(),
                kind: dialog.kind,
                loader_version: dialog.selected.clone(),
                instance_id: instance_id.clone(),
                launch_after: false,
            },
            cx,
        );

        let paths = self.paths.clone();
        let threads = self.settings.download_threads;
        let loader_kind = dialog.kind;
        let loader_version = dialog.selected.clone();
        self.spawn_engine(
            task_id,
            move |sender, cancel| {
                let version_id = if loader_kind.is_vanilla() {
                    game.clone()
                } else {
                    loader::install_loader(
                        &paths,
                        loader_kind,
                        &game,
                        loader_version.as_deref().unwrap_or(""),
                    )
                    .map_err(|e| e.to_string())?
                };
                let _ = sender.send(TaskEvent::Log(format!("__resolved__{version_id}")));
                install::install_version(&paths, &version_id, threads, sender, cancel)
                    .map(|_| ())
                    .map_err(|e| e.to_string())
            },
            cx,
        );
    }

    /// 启动实例，缺文件时先装。
    pub fn play(&mut self, instance_id: &str, cx: &mut Context<Self>) {
        let Some(instance) = self.instances.get(instance_id).cloned() else {
            return;
        };
        if self.running_instance.is_some() {
            let msg = self.t("toast.already_running").to_string();
            self.set_toast(msg, true, cx);
            return;
        }
        let Some(account) = self.settings.active_account().cloned() else {
            let msg = self.t("toast.need_account").to_string();
            self.set_toast(msg, true, cx);
            self.page = Page::Settings;
            return;
        };

        let installed = self.installed_versions().contains(&instance.version);
        if !installed {
            let (kind, loader_version) = parse_loader(&instance);
            let task_id = self.push_task(
                format!("Install {}", instance.version),
                instance.name.clone(),
                TaskKind::Install {
                    game: instance.version.clone(),
                    kind,
                    loader_version: loader_version.clone(),
                    instance_id: instance.id.clone(),
                    launch_after: true,
                },
                cx,
            );
            let paths = self.paths.clone();
            let threads = self.settings.download_threads;
            let game = instance.version.clone();
            self.spawn_engine(
                task_id,
                move |sender, cancel| {
                    let version_id = if kind.is_vanilla() {
                        game.clone()
                    } else {
                        loader::install_loader(
                            &paths,
                            kind,
                            &game,
                            loader_version.as_deref().unwrap_or(""),
                        )
                        .map_err(|e| e.to_string())?
                    };
                    let _ = sender.send(TaskEvent::Log(format!("__resolved__{version_id}")));
                    install::install_version(&paths, &version_id, threads, sender, cancel)
                        .map(|_| ())
                        .map_err(|e| e.to_string())
                },
                cx,
            );
            return;
        }

        self.running_instance = Some(instance.id.clone());
        let task_id = self.push_task(
            format!("Launch {}", instance.name),
            instance.display_version(),
            TaskKind::Run {
                instance: instance.id.clone(),
            },
            cx,
        );

        let paths = self.paths.clone();
        let settings = self.settings.clone();
        let instance = instance.clone();
        self.spawn_engine(
            task_id,
            move |sender, cancel| {
                run_instance(&paths, &settings, &instance, &account, sender, cancel)
            },
            cx,
        );
    }

    // ------------------------------------------------------------------ login

    pub fn start_microsoft_login(&mut self, cx: &mut Context<Self>) {
        if matches!(self.login, LoginView::Device { .. } | LoginView::Busy(_)) {
            return;
        }
        self.login = LoginView::Busy(self.t("login.busy").to_string());
        cx.notify();

        let cancel = cancel_token();
        let bridge = bridge();
        let sender = bridge.sender;
        let cancel_for_work = Arc::clone(&cancel);
        std::thread::spawn(move || {
            let config = auth::MsApp::default();
            let result = auth::microsoft_login(
                &config,
                &cancel_for_work,
                |device| {
                    let _ = sender.send(TaskEvent::Log(format!(
                        "__device__{}\u{1}{}\u{1}{}",
                        device.user_code, device.verification_uri, device.message
                    )));
                },
                |hint| {
                    let _ = sender.send(TaskEvent::Log(format!("__wait__{hint}")));
                },
            );
            match result {
                Ok(account) => {
                    let _ = sender.send(TaskEvent::Log(format!(
                        "__account__{}",
                        serde_json::to_string(&account).unwrap_or_default()
                    )));
                }
                Err(err) => {
                    let _ = sender.send(TaskEvent::Failed(err.to_string()));
                }
            }
        });

        let mut receiver = bridge.receiver;
        cx.spawn(async move |this, cx| {
            while let Some(event) = receiver.next().await {
                let stop = this
                    .update(cx, |app: &mut Self, cx| match event {
                        TaskEvent::Log(line) => {
                            if let Some(rest) = line.strip_prefix("__device__") {
                                let mut parts = rest.split('\u{1}');
                                let code = parts.next().unwrap_or_default().to_string();
                                let uri = parts.next().unwrap_or_default().to_string();
                                let hint = parts.next().unwrap_or_default().to_string();
                                app.login = LoginView::Device { code, uri, hint };
                                cx.notify();
                                false
                            } else if let Some(hint) = line.strip_prefix("__wait__") {
                                if let LoginView::Device { hint: h, .. } = &mut app.login {
                                    *h = hint.to_string();
                                }
                                cx.notify();
                                false
                            } else if let Some(json) = line.strip_prefix("__account__") {
                                if let Ok(account) = serde_json::from_str::<Account>(json) {
                                    app.settings.set_account(account.clone());
                                    let _ = app.settings.save(&app.paths);
                                    app.login = LoginView::Hidden;
                                    let msg = app.tf("toast.login_ok", &[("name", &account.name)]);
                                    app.set_toast(msg, false, cx);
                                }
                                true
                            } else {
                                false
                            }
                        }
                        TaskEvent::Failed(err) => {
                            app.login = LoginView::Failed(err.clone());
                            app.set_toast(err, true, cx);
                            true
                        }
                        _ => false,
                    })
                    .unwrap_or(true);
                if stop {
                    break;
                }
            }
        })
        .detach();
    }

    pub fn login_offline(&mut self, cx: &mut Context<Self>) {
        let name = self.offline_name.read(cx).value().trim().to_string();
        if name.is_empty() || name.len() > 16 {
            let msg = self.t("toast.name_invalid").to_string();
            self.set_toast(msg, true, cx);
            return;
        }
        let account = Account::offline(&name);
        self.settings.set_account(account.clone());
        let _ = self.settings.save(&self.paths);
        self.login = LoginView::Hidden;
        let msg = self.tf("toast.login_ok", &[("name", &account.name)]);
        self.set_toast(msg, false, cx);
    }

    pub fn logout(&mut self, name: &str, cx: &mut Context<Self>) {
        self.settings.remove_account(name);
        let _ = self.settings.save(&self.paths);
        cx.notify();
    }

    // ------------------------------------------------------------- instances

    pub fn delete_instance(&mut self, id: &str, cx: &mut Context<Self>) {
        if let Some(instance) = self.instances.remove(id) {
            let dir = self.paths.instance_dir(&instance.id);
            let _ = std::fs::remove_dir_all(dir);
            let _ = self.instances.save();
            if self.selected_instance.as_deref() == Some(id) {
                self.selected_instance = self.instances.list().first().map(|i| i.id.clone());
            }
            if self.page == Page::Instance {
                self.page = Page::Home;
            }
            let msg = self.tf("toast.deleted", &[("name", &instance.name)]);
            self.set_toast(msg, false, cx);
        }
        self.confirm_delete = None;
        cx.notify();
    }

    pub fn update_instance<F>(&mut self, id: &str, f: F, cx: &mut Context<Self>)
    where
        F: FnOnce(&mut Instance),
    {
        if let Some(instance) = self.instances.get_mut(id) {
            f(instance);
            let _ = self.instances.save();
            cx.notify();
        }
    }

    pub fn open_instance_page(&mut self, id: &str, cx: &mut Context<Self>) {
        self.selected_instance = Some(id.to_string());
        self.page = Page::Instance;
        cx.notify();
    }

    // ------------------------------------------------------------------- mods

    pub fn mods_dir(&self, instance_id: &str) -> PathBuf {
        self.paths.instance_dir(instance_id).join("mods")
    }

    pub fn list_mods(&self, instance_id: &str) -> Vec<ModEntry> {
        let dir = self.mods_dir(instance_id);
        let Ok(entries) = std::fs::read_dir(&dir) else {
            return Vec::new();
        };
        let mut mods: Vec<ModEntry> = entries
            .flatten()
            .filter(|e| e.path().is_file())
            .filter_map(|entry| {
                let name = entry.file_name().to_string_lossy().to_string();
                let lower = name.to_lowercase();
                let enabled = if lower.ends_with(".jar") {
                    true
                } else if lower.ends_with(".jar.disabled") {
                    false
                } else {
                    return None;
                };
                let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                Some(ModEntry {
                    file_name: name,
                    enabled,
                    size,
                })
            })
            .collect();
        mods.sort_by(|a, b| a.file_name.cmp(&b.file_name));
        mods
    }

    pub fn toggle_mod(&mut self, instance_id: &str, file_name: &str, cx: &mut Context<Self>) {
        let dir = self.mods_dir(instance_id);
        let from = dir.join(file_name);
        let to = if file_name.to_lowercase().ends_with(".disabled") {
            dir.join(file_name.trim_end_matches(".disabled"))
        } else {
            dir.join(format!("{file_name}.disabled"))
        };
        if std::fs::rename(&from, &to).is_ok() {
            cx.notify();
        }
    }

    pub fn delete_mod(&mut self, instance_id: &str, file_name: &str, cx: &mut Context<Self>) {
        let path = self.mods_dir(instance_id).join(file_name);
        if std::fs::remove_file(path).is_ok() {
            cx.notify();
        }
    }

    pub fn open_dir(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let _ = std::fs::create_dir_all(&path);
        if open::that(&path).is_err() {
            let msg = self.t("toast.open_failed").to_string();
            self.set_toast(msg, true, cx);
        }
    }

    // ------------------------------------------------------------------ misc

    pub fn account(&self) -> Option<&Account> {
        self.settings.active_account()
    }

    pub fn running_count(&self) -> usize {
        self.tasks.iter().filter(|t| t.is_running()).count()
    }

    pub fn cancel_task(&mut self, id: u64, cx: &mut Context<Self>) {
        let lang = self.lang();
        if let Some(task) = self.tasks.iter_mut().find(|t| t.id == id) {
            task.cancel.store(true, Ordering::Relaxed);
            task.status = TaskStatus::Cancelled;
            task.stage = i18n::tr(lang, "tasks.status.cancelled").to_string();
        }
        cx.notify();
    }

    pub fn clear_finished_tasks(&mut self) {
        self.tasks.retain(|t| t.is_running());
    }

    pub fn open_data_dir(&mut self, cx: &mut Context<Self>) {
        let dir = self.paths.root().to_path_buf();
        self.open_dir(dir, cx);
    }

    pub fn installed_versions(&self) -> Vec<String> {
        let mut out = Vec::new();
        if let Ok(entries) = std::fs::read_dir(self.paths.versions_dir()) {
            for entry in entries.flatten() {
                if entry.path().join("natives").is_dir() {
                    out.push(entry.file_name().to_string_lossy().to_string());
                }
            }
        }
        out
    }

    // ---------------------------------------------------------------- render

    fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let mut items = div().flex().flex_col().gap_1().px_3();
        for page in Page::nav_items() {
            let active = self.page == page;
            let mut item = div()
                .id(("nav", page as usize))
                .flex()
                .flex_row()
                .items_center()
                .gap_3()
                .h(px(40.))
                .px_3()
                .rounded(px(10.))
                .cursor_pointer()
                .text_sm()
                .child(
                    div()
                        .w(px(18.))
                        .text_center()
                        .text_color(if active {
                            theme.primary
                        } else {
                            theme.muted_foreground
                        })
                        .child(page.glyph()),
                )
                .child(div().child(self.t(page.key())));

            if active {
                item = item
                    .bg(theme.secondary)
                    .text_color(theme.foreground)
                    .font_weight(FontWeight::MEDIUM);
            } else {
                item = item.text_color(theme.muted_foreground);
            }

            items = items.child(item.on_click(cx.listener(move |this, _, _, cx| {
                this.page = page;
                cx.notify();
            })));
        }

        let account_label = self
            .account()
            .map(|a| a.name.clone())
            .unwrap_or_else(|| "—".to_string());
        let account_kind = match self.account() {
            Some(a) if a.is_microsoft() => self.t("settings.microsoft"),
            Some(_) => self.t("settings.offline"),
            None => self.t("settings.no_account"),
        };

        div()
            .w(px(216.))
            .h_full()
            .flex()
            .flex_col()
            .justify_between()
            .border_r_1()
            .border_color(theme.border)
            .bg(theme.sidebar)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_6()
                    .pt_5()
                    .child(
                        div()
                            .px_6()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .size(px(28.))
                                    .rounded(px(9.))
                                    .bg(theme.primary)
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_color(theme.primary_foreground)
                                    .font_weight(FontWeight::BOLD)
                                    .child("S"),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child(self.t("app.name")),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(theme.muted_foreground)
                                            .child(self.t("app.subtitle")),
                                    ),
                            ),
                    )
                    .child(items),
            )
            .child(
                div().p_3().child(
                    div()
                        .id("account-card")
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_3()
                        .p_2()
                        .rounded(px(10.))
                        .cursor_pointer()
                        .hover(|s| s.bg(theme.secondary))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.page = Page::Settings;
                            cx.notify();
                        }))
                        .child(
                            div()
                                .size(px(32.))
                                .rounded_full()
                                .bg(theme.secondary)
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_sm()
                                .child(ui::initial(&account_label)),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .child(div().text_sm().child(account_label))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(theme.muted_foreground)
                                        .child(account_kind),
                                ),
                        ),
                ),
            )
    }

    fn render_toast(&self, toast: Toast, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let color = if toast.error { theme.red } else { theme.green };
        div()
            .absolute()
            .bottom(px(24.))
            .left(px(240.))
            .right(px(24.))
            .flex()
            .child(
                div()
                    .px_4()
                    .py_2()
                    .rounded(px(10.))
                    .border_1()
                    .border_color(color)
                    .bg(theme.popover)
                    .text_sm()
                    .text_color(theme.popover_foreground)
                    .child(toast.message),
            )
    }

    fn render_page(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        match self.page {
            Page::Home => self.render_home(window, cx).into_any_element(),
            Page::Versions => self.render_versions(window, cx).into_any_element(),
            Page::Instance => self.render_instance(window, cx).into_any_element(),
            Page::Tasks => self.render_tasks(window, cx).into_any_element(),
            Page::Settings => self.render_settings(window, cx).into_any_element(),
        }
    }
}

impl Render for LauncherApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let mut root = div()
            .size_full()
            .flex()
            .flex_row()
            .bg(theme.background)
            .text_color(theme.foreground)
            .child(self.render_sidebar(cx))
            .child(div().flex_1().h_full().child(self.render_page(window, cx)));

        if let Some(toast) = self.toast.clone() {
            root = root.child(self.render_toast(toast, cx));
        }
        if let Some(dialog) = self.install_dialog.clone() {
            root = root.child(self.render_install_dialog(&dialog, cx));
        }
        if let Some((title, command)) = self.command_preview.clone() {
            root = root.child(self.render_command_dialog(title, command, cx));
        }
        if let Some(id) = self.confirm_delete.clone() {
            root = root.child(self.render_delete_dialog(id, cx));
        }
        root
    }
}

impl LauncherApp {
    fn render_install_dialog(
        &self,
        dialog: &InstallDialog,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme();
        let version = dialog.version.clone();

        let mut loader_row = div().flex().flex_row().gap_2();
        for kind in LoaderKind::all() {
            let selected = dialog.kind == kind;
            let label = match kind {
                LoaderKind::Vanilla => self.t("loader.vanilla"),
                LoaderKind::Fabric => self.t("loader.fabric"),
                LoaderKind::Quilt => self.t("loader.quilt"),
            };
            loader_row = loader_row.child(
                Button::new(SharedString::from(format!("loader-{}", kind.key())))
                    .label(label)
                    .selected(selected)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.choose_loader(kind, cx);
                    })),
            );
        }

        let mut body = div()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child(self.t("loader.desc")),
            )
            .child(loader_row);

        if !dialog.kind.is_vanilla() {
            if dialog.loading {
                body = body.child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(self.t("common.loading")),
                );
            } else if let Some(error) = &dialog.error {
                body = body.child(div().text_xs().text_color(theme.red).child(error.clone()));
            } else {
                let mut list = div()
                    .id("loader-list")
                    .max_h(px(180.))
                    .flex()
                    .flex_col()
                    .gap_1()
                    .overflow_y_scroll();
                for entry in dialog.loader_versions.iter().take(40) {
                    let selected = dialog.selected.as_deref() == Some(entry.version.as_str());
                    let version = entry.version.clone();
                    let label = if entry.stable {
                        entry.version.clone()
                    } else {
                        format!("{} (beta)", entry.version)
                    };
                    list = list.child(
                        div()
                            .id(SharedString::from(format!("lv-{}", entry.version)))
                            .flex()
                            .flex_row()
                            .justify_between()
                            .px_3()
                            .py_1()
                            .rounded(px(8.))
                            .cursor_pointer()
                            .bg(if selected {
                                theme.secondary
                            } else {
                                theme.background
                            })
                            .text_xs()
                            .child(label)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.pick_loader_version(version.clone(), cx);
                            })),
                    );
                }
                body = body.child(list);
            }
        }

        let can_confirm = dialog.kind.is_vanilla() || dialog.selected.is_some();

        div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(gpui_kit::black().opacity(0.5))
            .child(
                div()
                    .w(px(480.))
                    .flex()
                    .flex_col()
                    .gap_4()
                    .p_5()
                    .rounded(px(16.))
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.popover)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(2.))
                            .child(
                                div()
                                    .text_base()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(format!("{} · {version}", self.t("loader.title"))),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child(self.t("loader.version")),
                            ),
                    )
                    .child(body)
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .justify_end()
                            .gap_2()
                            .child(
                                Button::new("install-cancel")
                                    .label(self.t("common.cancel"))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.close_install_dialog(cx);
                                    })),
                            )
                            .child(
                                Button::new("install-confirm")
                                    .primary()
                                    .label(self.t("versions.install"))
                                    .disabled(!can_confirm)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.confirm_install(cx);
                                    })),
                            ),
                    ),
            )
    }

    fn render_command_dialog(
        &self,
        title: String,
        command: String,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme();
        div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(gpui_kit::black().opacity(0.5))
            .child(
                div()
                    .w(px(760.))
                    .max_h(px(560.))
                    .flex()
                    .flex_col()
                    .gap_3()
                    .p_5()
                    .rounded(px(16.))
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.popover)
                    .child(
                        div()
                            .text_base()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(title),
                    )
                    .child(
                        div()
                            .id("cmd-scroll")
                            .flex_1()
                            .overflow_y_scroll()
                            .text_xs()
                            .font_family("monospace")
                            .text_color(theme.muted_foreground)
                            .child(command),
                    )
                    .child(
                        div().flex().flex_row().justify_end().child(
                            Button::new("close-cmd")
                                .label(self.t("common.close"))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.command_preview = None;
                                    cx.notify();
                                })),
                        ),
                    ),
            )
    }

    fn render_delete_dialog(&self, id: String, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let name = self
            .instances
            .get(&id)
            .map(|i| i.name.clone())
            .unwrap_or_else(|| id.clone());
        div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(gpui_kit::black().opacity(0.5))
            .child(
                div()
                    .w(px(420.))
                    .flex()
                    .flex_col()
                    .gap_4()
                    .p_5()
                    .rounded(px(16.))
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.popover)
                    .child(
                        div()
                            .text_base()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(self.tf("dialog.delete_title", &[("name", &name)])),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .child(self.t("dialog.delete_desc")),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .justify_end()
                            .gap_2()
                            .child(
                                Button::new("cancel-delete")
                                    .label(self.t("common.cancel"))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.confirm_delete = None;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("confirm-delete")
                                    .label(self.t("common.delete"))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        let id = id.clone();
                                        this.delete_instance(&id, cx);
                                    })),
                            ),
                    ),
            )
    }
}

/// 从实例记录里还原加载器信息（`Fabric 0.16.9` -> (Fabric, "0.16.9")）。
fn parse_loader(instance: &Instance) -> (LoaderKind, Option<String>) {
    let Some(label) = instance.loader.as_deref() else {
        return (LoaderKind::Vanilla, None);
    };
    let mut parts = label.split_whitespace();
    let name = parts.next().unwrap_or("").to_lowercase();
    let version = parts.next().map(|s| s.to_string());
    let kind = match name.as_str() {
        "fabric" => LoaderKind::Fabric,
        "quilt" => LoaderKind::Quilt,
        _ => LoaderKind::Vanilla,
    };
    (kind, version)
}

/// 工作线程：解析版本、按需安装、挑 Java、启动进程。
fn run_instance(
    paths: &LauncherPaths,
    settings: &LauncherSettings,
    instance: &Instance,
    account: &Account,
    sender: Sender<TaskEvent>,
    cancel: Arc<std::sync::atomic::AtomicBool>,
) -> std::result::Result<(), String> {
    let _ = sender.send(TaskEvent::Stage("Checking game files".into()));
    let resolved = install::resolve_version(paths, &instance.version).map_err(|e| e.to_string())?;

    if !paths.version_jar(&instance.version).is_file() {
        install::install_version(
            paths,
            &instance.version,
            settings.download_threads,
            sender.clone(),
            Arc::clone(&cancel),
        )
        .map_err(|e| e.to_string())?;
    }

    let _ = sender.send(TaskEvent::Stage("Preparing account".into()));
    let account = auth::ensure_valid(&auth::MsApp::default(), account).map_err(|e| e.to_string())?;

    let _ = sender.send(TaskEvent::Stage("Looking for Java".into()));
    let required = resolved.required_java_major();
    let manual = instance
        .java_path
        .as_ref()
        .map(PathBuf::from)
        .filter(|p| p.is_file())
        .or_else(|| {
            settings
                .java_path
                .as_ref()
                .map(PathBuf::from)
                .filter(|p| p.is_file())
        });
    // 手动指定的优先；否则用本机的，实在没有就自动下一个 Temurin JRE。
    let java = match manual {
        Some(path) => path,
        None => java::ensure(required, paths, &sender, &cancel)
            .map_err(|e| e.to_string())?
            .path,
    };

    let extra_jvm: Vec<String> = instance
        .jvm_args
        .split_whitespace()
        .map(|s| s.to_string())
        .collect();
    let extra_game: Vec<String> = instance
        .game_args
        .split_whitespace()
        .map(|s| s.to_string())
        .collect();

    let options = LaunchOptions {
        paths,
        instance,
        version: &resolved,
        account: &account,
        java: &java,
        memory_mb: if instance.memory_mb > 0 {
            instance.memory_mb
        } else {
            settings.global_memory_mb
        },
        extra_jvm_args: extra_jvm,
        extra_game_args: extra_game,
    };

    let _ = sender.send(TaskEvent::Stage("Starting".into()));
    let process = launch::launch(&options, sender.clone()).map_err(|e| e.to_string())?;
    // 哨兵行：UI 收到它就说明游戏进程已经起来了，与界面语言无关。
    let _ = sender.send(TaskEvent::Log("__launched__".to_string()));
    let _ = sender.send(TaskEvent::Log(format!("$ {}", process.command_line)));
    let handle = process.watch(sender.clone());
    let code = handle.join().unwrap_or(-1);
    if code == 0 {
        Ok(())
    } else {
        Err(format!("Game exited with code {code}"))
    }
}
