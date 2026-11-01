//! 极简 i18n：一张静态表 + 占位符替换，不引入额外依赖。
//!
//! 键名缺失时回退到键名本身，方便开发时发现漏翻。

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lang {
    ZhCn,
    EnUs,
    JaJp,
}

impl Lang {
    pub fn all() -> [Lang; 3] {
        [Lang::ZhCn, Lang::EnUs, Lang::JaJp]
    }

    pub fn code(&self) -> &'static str {
        match self {
            Lang::ZhCn => "zh-CN",
            Lang::EnUs => "en-US",
            Lang::JaJp => "ja-JP",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Lang::ZhCn => "简体中文",
            Lang::EnUs => "English",
            Lang::JaJp => "日本語",
        }
    }

    pub fn short(&self) -> &'static str {
        match self {
            Lang::ZhCn => "中",
            Lang::EnUs => "EN",
            Lang::JaJp => "日",
        }
    }

    pub fn from_code(code: &str) -> Lang {
        match code {
            "en-US" | "en" => Lang::EnUs,
            "ja-JP" | "ja" => Lang::JaJp,
            _ => Lang::ZhCn,
        }
    }

    /// 首次启动时按系统语言猜一个。
    pub fn detect() -> Lang {
        let locale = std::env::var("LANG")
            .or_else(|_| std::env::var("LC_ALL"))
            .unwrap_or_default()
            .to_lowercase();
        if locale.starts_with("ja") {
            Lang::JaJp
        } else if locale.starts_with("en") {
            Lang::EnUs
        } else {
            Lang::ZhCn
        }
    }
}

/// (key, 简体中文, English, 日本語)
type Entry = (&'static str, &'static str, &'static str, &'static str);

const TABLE: &[Entry] = &[
    // ---------------- 通用 / shell
    ("app.name", "Stellae Craft", "Stellae Craft", "Stellae Craft"),
    ("app.subtitle", "启动器", "Launcher", "ランチャー"),
    ("nav.home", "启动台", "Play", "ホーム"),
    ("nav.versions", "版本库", "Versions", "バージョン"),
    ("nav.tasks", "任务中心", "Tasks", "タスク"),
    ("nav.settings", "设置", "Settings", "設定"),
    ("common.cancel", "取消", "Cancel", "キャンセル"),
    ("common.delete", "删除", "Delete", "削除"),
    ("common.close", "关闭", "Close", "閉じる"),
    ("common.confirm", "确定", "Confirm", "決定"),
    ("common.retry", "重试", "Retry", "再試行"),
    ("common.loading", "加载中…", "Loading…", "読み込み中…"),
    ("common.never", "从未启动", "Never played", "未プレイ"),
    ("common.minutes", "{n} 分钟", "{n} min", "{n} 分"),
    ("common.hours_minutes", "{h} 小时 {m} 分", "{h} h {m} min", "{h} 時間 {m} 分"),
    // ---------------- home
    ("home.welcome", "欢迎回来，{name}", "Welcome back, {name}", "おかえりなさい、{name}"),
    ("home.subtitle", "选一个实例，按下启动，星光就在前方。", "Pick an instance and launch. The stars are waiting.", "インスタンスを選んで起動しましょう。"),
    ("home.new_instance", "新建实例", "New instance", "新規インスタンス"),
    ("home.all_instances", "全部实例 · {n}", "All instances · {n}", "すべてのインスタンス · {n}"),
    ("home.empty_title", "还没有任何实例", "No instances yet", "インスタンスがありません"),
    ("home.empty_desc", "去版本库挑一个版本，Stellae Craft 会帮你把一切准备好。", "Browse the version library — Stellae Craft prepares everything for you.", "バージョン一覧から選べば、必要なものはすべて自動で用意されます。"),
    ("home.browse", "浏览版本库", "Browse versions", "バージョンを見る"),
    ("home.launch", "启动游戏", "Launch", "ゲームを起動"),
    ("home.running", "游戏运行中…", "Game running…", "ゲーム実行中…"),
    ("home.manage", "管理", "Manage", "管理"),
    ("home.preview", "启动命令", "Command", "起動コマンド"),
    ("home.delete", "删除", "Delete", "削除"),
    ("home.memory", "{n} MB 内存", "{n} MB RAM", "{n} MB メモリ"),
    ("home.hint_first_launch", "首次启动会自动补齐缺失的游戏文件", "Missing files are fetched automatically on first launch", "初回起動時に不足ファイルを自動で取得します"),
    ("home.hint_running", "退出游戏后可以再次启动", "Launch again after the game exits", "ゲーム終了後に再起動できます"),
    // ---------------- versions
    ("versions.title", "版本库", "Version library", "バージョン一覧"),
    ("versions.subtitle", "来自 Mojang 官方清单，安装后即可创建实例。", "Straight from Mojang's manifest. Install to create an instance.", "Mojang 公式マニフェストから取得します。"),
    ("versions.refresh", "刷新清单", "Refresh", "更新"),
    ("versions.refreshing", "正在刷新…", "Refreshing…", "更新中…"),
    ("versions.search", "搜索版本，例如 1.21", "Search versions, e.g. 1.21", "バージョンを検索（例: 1.21）"),
    ("versions.release", "正式版", "Release", "リリース"),
    ("versions.snapshot", "快照", "Snapshot", "スナップショット"),
    ("versions.all", "全部", "All", "すべて"),
    ("versions.install", "安装", "Install", "インストール"),
    ("versions.installed", "已安装", "Installed", "導入済み"),
    ("versions.open", "打开", "Open", "開く"),
    ("versions.load_more", "显示更多（{shown} / {total}）", "Show more ({shown} / {total})", "さらに表示（{shown} / {total}）"),
    ("versions.fetching", "正在获取版本清单…", "Fetching version manifest…", "バージョン一覧を取得中…"),
    ("versions.error", "清单获取失败：{error}", "Manifest failed: {error}", "取得に失敗しました: {error}"),
    ("versions.type.release", "正式版", "Release", "リリース"),
    ("versions.type.snapshot", "快照", "Snapshot", "スナップショット"),
    ("versions.type.old_beta", "远古 Beta", "Old Beta", "旧 Beta"),
    ("versions.type.old_alpha", "远古 Alpha", "Old Alpha", "旧 Alpha"),
    ("versions.type.other", "版本", "Version", "バージョン"),
    // ---------------- tasks
    ("tasks.title", "任务中心", "Tasks", "タスク"),
    ("tasks.summary", "{running} 个进行中 · {done} 个已结束", "{running} running · {done} finished", "{running} 件実行中 · {done} 件完了"),
    ("tasks.clear", "清空已结束", "Clear finished", "完了分を消去"),
    ("tasks.empty", "还没有任务，去版本库装一个吧", "No tasks yet — install a version first", "タスクはありません"),
    ("tasks.cancel", "取消", "Cancel", "中止"),
    ("tasks.status.running", "进行中", "Running", "実行中"),
    ("tasks.status.success", "已完成", "Done", "完了"),
    ("tasks.status.failed", "失败：{error}", "Failed: {error}", "失敗: {error}"),
    ("tasks.status.cancelled", "已取消", "Cancelled", "中止"),
    ("tasks.stage.preparing", "准备中", "Preparing", "準備中"),
    ("tasks.stage.done", "已完成", "Done", "完了"),
    ("tasks.java_title", "下载 Java {major}", "Download Java {major}", "Java {major} を取得"),
    // ---------------- settings
    ("settings.title", "设置", "Settings", "設定"),
    ("settings.subtitle", "账户、运行环境与性能参数。", "Accounts, runtime and performance.", "アカウント・実行環境・性能設定。"),
    ("settings.account", "账户", "Account", "アカウント"),
    ("settings.account_desc", "微软账户可进入正版服务器，离线账户仅适合单机。", "Microsoft accounts can join online servers; offline accounts are for single player.", "Microsoft アカウントはオンライン対応、オフラインはシングルのみ。"),
    ("settings.logout", "退出登录", "Sign out", "サインアウト"),
    ("settings.no_account", "还没有账户，先登录一个吧。", "No account yet — sign in first.", "アカウントがありません。"),
    ("settings.ms_login", "使用微软账户登录", "Sign in with Microsoft", "Microsoft でサインイン"),
    ("settings.offline_placeholder", "离线玩家名", "Offline player name", "オフラインプレイヤー名"),
    ("settings.offline_add", "添加离线账户", "Add offline account", "オフライン追加"),
    ("settings.microsoft", "微软账户", "Microsoft account", "Microsoft アカウント"),
    ("settings.offline", "离线账户", "Offline account", "オフラインアカウント"),
    ("settings.in_use", "使用中", "In use", "使用中"),
    ("settings.switch", "切换", "Use", "切替"),
    ("settings.java", "Java 运行时", "Java runtime", "Java ランタイム"),
    ("settings.java_desc", "不同版本对 Java 版本有要求，1.17+ 需要 Java 17 以上。", "Versions need different Java releases; 1.17+ needs Java 17 or newer.", "1.17 以降は Java 17 以上が必要です。"),
    ("settings.java_auto", "自动选择", "Automatic", "自動選択"),
    ("settings.java_auto_desc", "按版本要求挑选最合适的 Java", "Pick the best runtime for each version", "バージョンに合う Java を自動で選択"),
    ("settings.java_rescan", "重新扫描", "Rescan", "再スキャン"),
    ("settings.java_scanning", "扫描中…", "Scanning…", "スキャン中…"),
    ("settings.java_scanning_hint", "正在扫描本机 Java…", "Scanning local Java runtimes…", "Java をスキャン中…"),
    ("settings.java_none", "没有扫描到 Java，可以安装一个 JDK 17+ 后重新扫描。", "No Java found. Install a JDK 17+ and rescan.", "Java が見つかりません。JDK 17+ を入れて再スキャンしてください。"),
    ("settings.java_pick", "手动选择", "Pick manually", "手動で選択"),
    ("settings.java_picked", "已指定 {path}", "Using {path}", "{path} を使用中"),
    ("settings.java_download", "自动下载 Java", "Download Java", "Java を自動取得"),    ("settings.java_download_hint", "没装 Java？让启动器下载一个 Temurin JRE。", "No Java installed? Let the launcher fetch a Temurin JRE.", "Java が無い場合は Temurin JRE を取得できます。"),
    ("settings.java_managed", "启动器托管", "Managed", "ランチャー管理"),
    ("settings.perf", "性能", "Performance", "パフォーマンス"),
    ("settings.perf_desc", "新实例默认使用的内存与下载并发数。", "Default memory and download concurrency for new instances.", "新規インスタンスの既定メモリと同時接続数。"),
    ("settings.memory", "默认最大内存", "Default max memory", "既定の最大メモリ"),
    ("settings.threads", "下载并发", "Download threads", "同時ダウンロード数"),
    ("settings.close_on_launch", "启动后关闭启动器", "Close launcher after launch", "起動後にランチャーを閉じる"),
    ("settings.close_on_launch_desc", "启动游戏后自动退出，节省内存", "Quit the launcher once the game starts", "ゲーム起動後にランチャーを終了"),
    ("settings.about", "关于", "About", "このアプリについて"),
    ("settings.about_desc", "用 Rust + GPUI Kit 构建的全平台启动器", "A cross-platform launcher built with Rust and GPUI Kit", "Rust と GPUI Kit で作られたクロスプラットフォーム版"),
    ("settings.version", "版本 {v}", "Version {v}", "バージョン {v}"),
    ("settings.data_dir", "数据目录 {path}", "Data directory {path}", "データフォルダ {path}"),
    ("settings.platform", "平台 {os} · {arch}", "Platform {os} · {arch}", "プラットフォーム {os} · {arch}"),
    ("settings.open_data", "打开数据目录", "Open data folder", "データフォルダを開く"),
    ("settings.open_logs", "查看日志", "Open logs", "ログを開く"),
    ("settings.language", "界面语言", "Language", "表示言語"),
    ("settings.language_desc", "切换后立即生效。", "Takes effect immediately.", "すぐに反映されます。"),
    ("settings.network", "网络", "Network", "ネットワーク"),
    ("settings.network_desc", "下载源与代理，国内建议用 BMCLAPI。", "Download mirror and proxy. BMCLAPI is much faster in mainland China.", "ダウンロード元とプロキシ。中国本土では BMCLAPI が高速です。"),
    ("settings.mirror", "下载源", "Download source", "ダウンロード元"),
    ("settings.mirror_official", "官方", "Official", "公式"),
    ("settings.mirror_bmclapi", "BMCLAPI", "BMCLAPI", "BMCLAPI"),
    ("settings.proxy", "代理", "Proxy", "プロキシ"),
    ("settings.game_dir", "游戏目录", "Game folder", "ゲームフォルダ"),
    ("settings.game_dir_desc", "共用 assets / libraries / versions 的 .minecraft 目录，可以直接用官方启动器的。", "The .minecraft folder holding assets, libraries and versions. The official launcher's folder works too.", "assets・libraries・versions を共有する .minecraft。公式ランチャーのフォルダも使えます。"),
    ("settings.game_dir_default", "启动器默认", "Launcher default", "ランチャー既定"),
    ("settings.game_dir_add", "添加目录…", "Add folder…", "フォルダを追加…"),
    ("settings.game_dir_current", "当前使用 {path}", "Using {path}", "{path} を使用中"),
    ("settings.appearance", "外观", "Appearance", "外観"),    ("settings.appearance_desc", "深色适合夜里玩，浅色适合白天。", "Dark for late-night sessions, light for daytime.", "夜はダーク、昼はライトがおすすめです。"),
    ("settings.theme_dark", "深色", "Dark", "ダーク"),
    ("settings.theme_light", "浅色", "Light", "ライト"),
    // ---------------- login
    ("login.busy", "正在申请设备码…", "Requesting device code…", "デバイスコードを取得中…"),
    ("login.hint", "在浏览器中打开下面的地址，输入这段代码：", "Open the address below in a browser and enter this code:", "次の URL を開き、コードを入力してください:"),
    ("login.open_browser", "打开浏览器", "Open browser", "ブラウザを開く"),
    ("login.copy", "复制代码", "Copy code", "コードをコピー"),
    ("login.copied", "代码已复制", "Code copied", "コードをコピーしました"),
    ("login.waiting", "等待你在浏览器中完成授权…", "Waiting for you to approve in the browser…", "ブラウザでの承認を待っています…"),
    // ---------------- dialogs / toasts
    ("dialog.command_title", "启动命令预览", "Launch command preview", "起動コマンドのプレビュー"),
    ("dialog.delete_title", "删除实例「{name}」？", "Delete instance \"{name}\"?", "インスタンス「{name}」を削除しますか？"),
    ("dialog.delete_desc", "实例目录与存档会被永久删除，这个操作无法撤销。", "The instance folder and its worlds are removed permanently.", "インスタンスとワールドは完全に削除されます。"),
    ("toast.installed", "{version} 安装完成", "{version} installed", "{version} をインストールしました"),
    ("toast.login_ok", "欢迎回来，{name}", "Welcome back, {name}", "おかえりなさい、{name}"),
    ("toast.game_exited", "游戏已退出", "Game exited", "ゲームが終了しました"),
    ("toast.name_invalid", "玩家名需要 1-16 个字符", "Player name must be 1-16 characters", "名前は 1〜16 文字で入力してください"),
    ("toast.already_running", "已经有一个游戏在运行了", "A game is already running", "すでにゲームが実行中です"),
    ("toast.need_account", "请先登录一个账户", "Sign in first", "先にサインインしてください"),
    ("toast.java_missing", "没有找到可用的 Java {major}，请在设置里指定", "No Java {major} found — set one in Settings", "Java {major} が見つかりません。設定で指定してください"),
    ("toast.open_failed", "无法打开文件管理器", "Could not open the file manager", "ファイルマネージャを開けません"),
    ("toast.manifest_failed", "版本清单解析失败", "Could not parse the version manifest", "バージョン一覧の解析に失敗しました"),
    ("toast.deleted", "已删除 {name}", "Deleted {name}", "{name} を削除しました"),
    ("toast.java_ready", "Java {major} 已就绪", "Java {major} is ready", "Java {major} の準備ができました"),
    // ---------------- loader
    ("loader.title", "选择加载器", "Choose a loader", "ローダーを選択"),
    ("loader.desc", "原版最稳定；Fabric / Quilt 用来装 Mod。", "Vanilla is the safest; Fabric / Quilt are for mods.", "バニラが最も安定。Fabric / Quilt は Mod 用です。"),
    ("loader.vanilla", "原版", "Vanilla", "バニラ"),
    ("loader.fabric", "Fabric", "Fabric", "Fabric"),
    ("loader.quilt", "Quilt", "Quilt", "Quilt"),
    ("loader.version", "加载器版本", "Loader version", "ローダーのバージョン"),
    ("loader.latest", "最新", "Latest", "最新"),
    ("loader.installing", "正在安装加载器…", "Installing loader…", "ローダーをインストール中…"),
    ("loader.failed", "加载器安装失败：{error}", "Loader install failed: {error}", "ローダーの導入に失敗: {error}"),
    // ---------------- instance page
    ("instance.title", "实例管理", "Instance", "インスタンス管理"),
    ("instance.back", "返回启动台", "Back to Play", "ホームに戻る"),
    ("instance.overview", "概览", "Overview", "概要"),
    ("instance.memory", "内存", "Memory", "メモリ"),
    ("instance.java", "Java", "Java", "Java"),
    ("instance.mods", "Mod", "Mods", "Mod"),
    ("instance.mods_desc", "放在 mods 目录里的 .jar，禁用后会改名为 .disabled。", "Jars in the mods folder. Disabling renames them to .disabled.", "mods フォルダ内の .jar。無効化すると .disabled になります。"),
    ("instance.mods_empty", "还没有 Mod，点「打开目录」把 .jar 放进去。", "No mods yet. Open the folder and drop .jar files in.", "Mod がありません。フォルダに .jar を入れてください。"),
    ("instance.mod_enable", "启用", "Enable", "有効化"),
    ("instance.mod_disable", "禁用", "Disable", "無効化"),
    ("instance.open_dir", "打开实例目录", "Open instance folder", "インスタンスフォルダを開く"),
    ("instance.open_mods", "打开 mods 目录", "Open mods folder", "mods フォルダを開く"),
    ("instance.play_time", "游戏时长", "Play time", "プレイ時間"),
    ("instance.created", "创建于", "Created", "作成日"),
    ("instance.reinstall", "重新校验文件", "Verify files", "ファイルを検証"),
];

fn entry(key: &str) -> Option<&'static Entry> {
    TABLE.iter().find(|e| e.0 == key)
}

/// 简单取词，找不到就回退到 key。
pub fn tr(lang: Lang, key: &str) -> &'static str {
    match entry(key) {
        Some(e) => match lang {
            Lang::ZhCn => e.1,
            Lang::EnUs => e.2,
            Lang::JaJp => e.3,
        },
        None => "",
    }
}

/// 带 `{name}` 占位符的取词。
pub fn tf(lang: Lang, key: &str, args: &[(&str, &str)]) -> String {
    let mut text = tr(lang, key).to_string();
    if text.is_empty() {
        text = key.to_string();
    }
    for (name, value) in args {
        text = text.replace(&format!("{{{name}}}"), value);
    }
    text
}
