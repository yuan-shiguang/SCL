# Stellae Craft Launcher

用 **Rust + [GPUI Kit](https://gpui-kit.com)** 写的 Minecraft 启动器，一套代码覆盖
Windows / macOS / Linux。界面由 GPU 直接渲染，没有 WebView、没有 Electron。

> Stellae = 拉丁语「星辰」，可执行文件名 `stellae-craft`。

---

## 功能

| 能力 | 说明 |
| --- | --- |
| 微软账户登录 | OAuth 设备码 → Xbox Live → XSTS → Minecraft Services，走自己注册的 Azure 应用 |
| 离线账户 | 按 Java 规则生成 `OfflinePlayer:<name>` 的 UUID |
| 版本安装 | Mojang `version_manifest_v2`，解析 `rules` / `natives` / `arguments`，并发下载 + SHA-1 校验 |
| 加载器 | Fabric、Quilt 一键安装（拉 meta 的 profile JSON，复用原版安装流水线） |
| 版本继承 | 支持 `inheritsFrom`，加载器版本自动合并父版本 |
| 启动游戏 | 拼 JVM / 游戏参数，解压本地库，接管 stdout/stderr 到日志面板 |
| Java 探测 | 扫 `JAVA_HOME`、PATH、Program Files、`/Library/Java`、`/usr/lib/jvm`，按版本要求自动挑 |
| Java 自动下载 | 本机没有合适的运行时，就从 Adoptium 拉一个 Temurin JRE 到 `java/`（SHA-256 校验） |
| 实例管理 | 每实例独立游戏目录、内存、Java；Mod 列表可启用/禁用/删除 |
| 任务中心 | 下载进度、当前文件、日志尾部、可取消 |
| 多语言 | 简体中文 / English / 日本語，设置里即时切换 |

## 目录结构

```
crates/
  scl-core/          与 UI 无关的引擎
    auth.rs          微软 / 离线账户
    meta.rs          版本清单、rules、Maven 坐标、inheritsFrom 合并
    loader.rs        Fabric / Quilt 安装
    install.rs       安装流水线（核心 + 运行库 + natives + assets）
    download.rs      多线程下载器 + SHA-1 校验 + 进度事件
    launch.rs        命令行拼装、natives、进程托管
    java.rs          Java 运行时探测
    instance.rs      实例模型与持久化
    config.rs        全局设置与账户
    paths.rs         各平台数据目录
  scl-app/           GPUI Kit 界面
    app.rs           应用状态、任务编排、侧边栏、对话框
    i18n.rs          静态词典（zh-CN / en-US / ja-JP）
    views/           启动台 / 版本库 / 实例管理 / 任务中心 / 设置
    task.rs          std::mpsc → futures channel 的桥接
```

引擎发 `TaskEvent`（Stage / Progress / Log / Done / Failed），UI 用 `cx.spawn`
消费并 `cx.notify()`，所以下载永远不会卡住界面。

## 构建

```bash
cargo run --release
```

首次构建要编译 GPUI / wgpu，十几分钟起步。

### 环境要求

- **Windows**：MSVC 工具链（推荐）或 MinGW-w64
- **macOS**：Xcode Command Line Tools
- **Linux**：`libwayland-dev libxkbcommon-dev libvulkan-dev libssl-dev libfontconfig1-dev libfreetype6-dev libasound2-dev`

仓库本身**不含任何机器相关配置**。如果本机需要特殊设置，在本地创建下面两个文件
（都已在 `.gitignore` 里）：

```toml
# .cargo/config.toml —— 例如国内镜像 + 指定链接器
[source.crates-io]
replace-with = "rsproxy-sparse"

[source.rsproxy-sparse]
registry = "sparse+https://rsproxy.cn/index/"

[target.x86_64-pc-windows-gnu]
linker = "C:\\Qt\\Tools\\mingw1310_64\\bin\\gcc.exe"
ar = "C:\\Qt\\Tools\\mingw1310_64\\bin\\ar.exe"
```

```toml
# rust-toolchain.toml —— 例如没装 MSVC 时改用 GNU 工具链
[toolchain]
channel = "stable-x86_64-pc-windows-gnu"
```

### CI

`.github/workflows/build.yml` 会在 push / PR 时跑：

1. `cargo test -p scl-core`（引擎单元测试）
2. Windows / macOS / Linux 三平台 `cargo build --release`
3. 打包成 zip / tar.gz 上传 artifact
4. 打 `v*` tag 时自动发 Release

## 微软登录

`crates/scl-core/src/auth.rs` 里是本项目在 Azure 注册的应用：

```rust
pub const CLIENT_ID: &str = "3d71cd0f-26f1-4d3a-9889-cdbe20506ace";
pub const TENANT_ID: &str = "993f705b-ab92-4f24-92b7-36415614dc5f";
```

登录走设备码流程：点「使用微软账户登录」后界面显示 8 位代码和
`microsoft.com/link`，浏览器里确认即可，启动器轮询拿 token。

租户默认用 `common`，这样个人微软账号才能登录；如果应用被限制成
「仅此组织目录」，设置环境变量：

```bash
SCL_MS_TENANT=993f705b-ab92-4f24-92b7-36415614dc5f cargo run
```

## 数据目录

| 平台 | 路径 |
| --- | --- |
| Windows | `%APPDATA%\StellaeCraft` |
| macOS | `~/Library/Application Support/StellaeCraft` |
| Linux | `~/.local/share/StellaeCraft` |

```
StellaeCraft/
  launcher.json      设置 + 账户 + 语言
  instances.json     实例注册表
  minecraft/         共享的 assets / libraries / versions
  instances/<id>/    每个实例的游戏目录（mods、saves、options.txt）
  logs/              启动日志
```

## 待办

- Forge / NeoForge 安装（需要跑 installer jar）
- 资源包 / 光影管理
- 自定义主题（现在跟随 GPUI Kit 的主题令牌）
