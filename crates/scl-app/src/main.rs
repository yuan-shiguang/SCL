//! Stellae Craft Launcher — a Minecraft launcher built with GPUI Kit.
//!
//! 一套代码，三个桌面平台：Windows / macOS / Linux。

mod app;
mod i18n;
mod task;
mod ui;
mod views;

use gpui_kit::*;

fn main() {
    // `stellae-craft --launch <实例 id>`：不开界面，直接把游戏拉起来并打印日志，
    // 排查「点了启动没反应」时很好用。
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("--launch") {
        let Some(instance) = args.get(2) else {
            eprintln!("用法: stellae-craft --launch <实例 id>");
            std::process::exit(2);
        };
        match app::launch_from_cli(instance) {
            Ok(()) => std::process::exit(0),
            Err(error) => {
                eprintln!("启动失败: {error}");
                std::process::exit(1);
            }
        }
    }

    application()
        .with_assets(assets::Assets)
        .run(|cx| {
            init(cx);

            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: point(px(120.), px(90.)),
                    size: size(px(1200.), px(800.)),
                })),
                window_min_size: Some(size(px(980.), px(640.))),
                ..Default::default()
            };

            let (handle, _view) = open_window(options, cx, |window, cx| {
                window.set_window_title("Stellae Craft Launcher");
                cx.new(|cx| app::LauncherApp::new(window, cx))
            })
            .expect("failed to open the main window");

            // 关掉最后一个窗口就退出。
            cx.on_window_closed(move |cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();
            let _ = handle;
        });
}
