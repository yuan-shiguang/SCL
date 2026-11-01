//! 设置：账户、Java、性能、语言、关于。

use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::Input;
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{ActiveTheme, Selectable};
use gpui_kit::*;

use crate::app::{LauncherApp, LoginView};
use crate::i18n::Lang;
use crate::ui;

const MEMORY_PRESETS: [u32; 6] = [2048, 4096, 6144, 8192, 12288, 16384];
const THREAD_PRESETS: [usize; 3] = [4, 8, 16];
/// 可以一键下载的 Java 大版本（1.20- 用 8，1.17-1.20 用 17，1.21+ 用 21）。
const JAVA_PRESETS: [u32; 3] = [8, 17, 21];

impl LauncherApp {
    pub fn render_settings(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme().clone();
        let mut page = div()
            .flex()
            .flex_col()
            .size_full()
            .p_8()
            .gap_5()
            .id("settings-scroll")
            .overflow_y_scroll()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_2xl()
                            .font_weight(FontWeight::BOLD)
                            .child(self.t("settings.title")),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .child(self.t("settings.subtitle")),
                    ),
            );

        page = page.child(self.render_account_section(&theme, cx));
        page = page.child(self.render_java_section(&theme, cx));
        page = page.child(self.render_performance_section(&theme, cx));
        page = page.child(self.render_language_section(&theme, cx));
        page = page.child(self.render_about_section(&theme, cx));
        page
    }

    fn section(
        &self,
        title: &'static str,
        subtitle: &'static str,
        theme: &gpui_kit::component::Theme,
    ) -> gpui_kit::Div {
        div()
            .flex()
            .flex_col()
            .gap(px(2.))
            .child(
                div()
                    .text_base()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(title),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(subtitle),
            )
    }

    fn render_account_section(
        &self,
        theme: &gpui_kit::component::Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let mut body = div().flex().flex_col().gap_4();
        let active = self.account().cloned();

        body = body.child(match &active {
            Some(account) => div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .p_3()
                .rounded(px(12.))
                .bg(theme.secondary)
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_3()
                        .child(
                            div()
                                .size(px(36.))
                                .rounded_full()
                                .bg(theme.primary)
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_sm()
                                .text_color(theme.primary_foreground)
                                .child(ui::initial(&account.name)),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::MEDIUM)
                                        .child(account.name.clone()),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(theme.muted_foreground)
                                        .child(if account.is_microsoft() {
                                            format!("{} · {}", self.t("settings.microsoft"), account.uuid)
                                        } else {
                                            self.t("settings.offline").to_string()
                                        }),
                                ),
                        ),
                )
                .child(
                    Button::new("logout-active")
                        .label(self.t("settings.logout"))
                        .on_click(cx.listener({
                            let name = account.name.clone();
                            move |this, _, _, cx| {
                                this.logout(&name, cx);
                            }
                        })),
                )
                .into_any_element(),
            None => div()
                .p_4()
                .rounded(px(12.))
                .border_1()
                .border_color(theme.border)
                .text_sm()
                .text_color(theme.muted_foreground)
                .child(self.t("settings.no_account"))
                .into_any_element(),
        });

        body = body.child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap_3()
                .child(
                    Button::new("ms-login")
                        .primary()
                        .label(self.t("settings.ms_login"))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.start_microsoft_login(cx);
                        })),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .child(div().w(px(160.)).child(Input::new(&self.offline_name)))
                        .child(
                            Button::new("offline-login")
                                .label(self.t("settings.offline_add"))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.login_offline(cx);
                                })),
                        ),
                ),
        );

        match &self.login {
            LoginView::Hidden => {}
            LoginView::Busy(message) => {
                body = body.child(
                    div()
                        .p_4()
                        .rounded(px(12.))
                        .bg(theme.secondary)
                        .text_sm()
                        .text_color(theme.muted_foreground)
                        .child(message.clone()),
                );
            }
            LoginView::Failed(error) => {
                body = body.child(
                    div()
                        .p_4()
                        .rounded(px(12.))
                        .border_1()
                        .border_color(theme.red)
                        .text_sm()
                        .text_color(theme.red)
                        .child(error.clone()),
                );
            }
            LoginView::Device { code, uri, hint } => {
                let uri_owned = uri.clone();
                let code_owned = code.clone();
                body = body.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .p_4()
                        .rounded(px(12.))
                        .border_1()
                        .border_color(theme.primary)
                        .bg(theme.secondary)
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child(self.t("login.hint")),
                        )
                        .child(
                            div()
                                .text_3xl()
                                .font_family("monospace")
                                .font_weight(FontWeight::BOLD)
                                .text_color(theme.primary)
                                .child(code.clone()),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child(uri.clone()),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .gap_2()
                                .child(
                                    Button::new("open-verify")
                                        .primary()
                                        .label(self.t("login.open_browser"))
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            if open::that(&uri_owned).is_err() {
                                                let msg = this.t("toast.open_failed").to_string();
                                                this.set_toast(msg, true, cx);
                                            }
                                        })),
                                )
                                .child(
                                    Button::new("copy-code")
                                        .label(self.t("login.copy"))
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            cx.write_to_clipboard(ClipboardItem::new_string(
                                                code_owned.clone(),
                                            ));
                                            let msg = this.t("login.copied").to_string();
                                            this.set_toast(msg, false, cx);
                                        })),
                                )
                                .child(
                                    Button::new("cancel-login")
                                        .label(self.t("common.cancel"))
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.login = LoginView::Hidden;
                                            cx.notify();
                                        })),
                                ),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child(if hint.is_empty() {
                                    self.t("login.waiting").to_string()
                                } else {
                                    hint.clone()
                                }),
                        ),
                );
            }
        }

        if self.settings.accounts.len() > 1 {
            let mut list = div().flex().flex_col().gap_2();
            for account in self.settings.accounts.clone() {
                let is_active = self
                    .settings
                    .active_account
                    .as_deref()
                    .map(|n| n == account.name)
                    .unwrap_or(false);
                let name = account.name.clone();
                let kind = if account.is_microsoft() {
                    self.t("settings.microsoft")
                } else {
                    self.t("settings.offline")
                };
                list = list.child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_between()
                        .px_3()
                        .py_2()
                        .rounded(px(10.))
                        .bg(if is_active {
                            theme.secondary
                        } else {
                            theme.background
                        })
                        .child(div().text_sm().child(format!("{}  ·  {}", account.name, kind)))
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .gap_2()
                                .child(
                                    Button::new(SharedString::from(format!("use-{name}")))
                                        .label(if is_active {
                                            self.t("settings.in_use")
                                        } else {
                                            self.t("settings.switch")
                                        })
                                        .on_click(cx.listener({
                                            let name = name.clone();
                                            move |this, _, _, cx| {
                                                this.settings.active_account = Some(name.clone());
                                                let _ = this.settings.save(&this.paths);
                                                cx.notify();
                                            }
                                        })),
                                )
                                .child(
                                    Button::new(SharedString::from(format!("rm-{name}")))
                                        .label(self.t("common.delete"))
                                        .on_click(cx.listener({
                                            let name = name.clone();
                                            move |this, _, _, cx| {
                                                this.logout(&name, cx);
                                            }
                                        })),
                                ),
                        ),
                );
            }
            body = body.child(list);
        }

        div()
            .flex()
            .flex_col()
            .gap_4()
            .p_5()
            .rounded(px(16.))
            .border_1()
            .border_color(theme.border)
            .child(self.section(
                self.t("settings.account"),
                self.t("settings.account_desc"),
                theme,
            ))
            .child(body)
    }

    fn render_java_section(
        &self,
        theme: &gpui_kit::component::Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let mut list = div().flex().flex_col().gap_2();
        let auto_selected = self.settings.java_path.is_none();

        list = list.child(
            div()
                .id("java-auto")
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .px_3()
                .py_2()
                .rounded(px(10.))
                .bg(if auto_selected {
                    theme.secondary
                } else {
                    theme.background
                })
                .cursor_pointer()
                .on_click(cx.listener(|this, _, _, cx| {
                    this.settings.java_path = None;
                    let _ = this.settings.save(&this.paths);
                    cx.notify();
                }))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .child(div().text_sm().child(self.t("settings.java_auto")))
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child(self.t("settings.java_auto_desc")),
                        ),
                )
                .child(if auto_selected {
                    ui::badge(self.t("settings.in_use"), theme.green, theme.background)
                        .into_any_element()
                } else {
                    div().into_any_element()
                }),
        );

        if self.javas.is_empty() {
            list = list.child(
                div()
                    .px_3()
                    .py_4()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(if self.java_scanning {
                        self.t("settings.java_scanning_hint")
                    } else {
                        self.t("settings.java_none")
                    }),
            );
        }

        for runtime in &self.javas {
            let path = runtime.path.to_string_lossy().to_string();
            let selected = self
                .settings
                .java_path
                .as_deref()
                .map(|p| p == path)
                .unwrap_or(false);
            let path_owned = path.clone();
            list = list.child(
                div()
                    .id(SharedString::from(format!("java-{path}")))
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .px_3()
                    .py_2()
                    .rounded(px(10.))
                    .bg(if selected {
                        theme.secondary
                    } else {
                        theme.background
                    })
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.settings.java_path = Some(path_owned.clone());
                        let _ = this.settings.save(&this.paths);
                        cx.notify();
                    }))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .text_sm()
                                    .child(format!("Java {} · {}", runtime.major, runtime.vendor)),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child(path.clone()),
                            ),
                    )
                    .child(if selected {
                        ui::badge(self.t("settings.in_use"), theme.green, theme.background)
                            .into_any_element()
                    } else {
                        div().into_any_element()
                    }),
            );
        }

        div()
            .flex()
            .flex_col()
            .gap_4()
            .p_5()
            .rounded(px(16.))
            .border_1()
            .border_color(theme.border)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .child(self.section(
                        self.t("settings.java"),
                        self.t("settings.java_desc"),
                        theme,
                    ))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_2()
                            .children(JAVA_PRESETS.iter().map(|major| {
                                let major = *major;
                                Button::new(SharedString::from(format!("get-java-{major}")))
                                    .label(format!("+ Java {major}"))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.download_java(major, cx);
                                    }))
                            }))
                            .child(
                                Button::new("rescan-java")
                                    .label(if self.java_scanning {
                                        self.t("settings.java_scanning")
                                    } else {
                                        self.t("settings.java_rescan")
                                    })
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.detect_java(cx);
                                    })),
                            ),
                    ),
            )
            .child(list)
    }

    fn render_performance_section(
        &self,
        theme: &gpui_kit::component::Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let mut memory_row = div().flex().flex_row().gap_2();
        for preset in MEMORY_PRESETS {
            let selected = self.settings.global_memory_mb == preset;
            memory_row = memory_row.child(
                Button::new(SharedString::from(format!("mem-{preset}")))
                    .label(format!("{} GB", preset / 1024))
                    .selected(selected)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.settings.global_memory_mb = preset;
                        let _ = this.settings.save(&this.paths);
                        cx.notify();
                    })),
            );
        }

        let mut thread_row = div().flex().flex_row().gap_2();
        for preset in THREAD_PRESETS {
            let selected = self.settings.download_threads == preset;
            thread_row = thread_row.child(
                Button::new(SharedString::from(format!("thread-{preset}")))
                    .label(format!("{preset}"))
                    .selected(selected)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.settings.download_threads = preset;
                        let _ = this.settings.save(&this.paths);
                        cx.notify();
                    })),
            );
        }

        div()
            .flex()
            .flex_col()
            .gap_4()
            .p_5()
            .rounded(px(16.))
            .border_1()
            .border_color(theme.border)
            .child(self.section(
                self.t("settings.perf"),
                self.t("settings.perf_desc"),
                theme,
            ))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(self.t("settings.memory")),
                    )
                    .child(memory_row),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(self.t("settings.threads")),
                    )
                    .child(thread_row),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(div().text_sm().child(self.t("settings.close_on_launch")))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child(self.t("settings.close_on_launch_desc")),
                            ),
                    )
                    .child(
                        Switch::new("close-on-launch")
                            .checked(self.settings.close_launcher_on_launch)
                            .on_change(cx.listener(|this, value, _, cx| {
                                this.settings.close_launcher_on_launch = *value;
                                let _ = this.settings.save(&this.paths);
                                cx.notify();
                            })),
                    ),
            )
    }

    fn render_language_section(
        &self,
        theme: &gpui_kit::component::Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let mut row = div().flex().flex_row().gap_2();
        for lang in Lang::all() {
            let selected = self.lang() == lang;
            row = row.child(
                Button::new(SharedString::from(format!("lang-{}", lang.code())))
                    .label(lang.label())
                    .selected(selected)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.set_language(lang, cx);
                    })),
            );
        }

        div()
            .flex()
            .flex_col()
            .gap_4()
            .p_5()
            .rounded(px(16.))
            .border_1()
            .border_color(theme.border)
            .child(self.section(
                self.t("settings.language"),
                self.t("settings.language_desc"),
                theme,
            ))
            .child(row)
    }

    fn render_about_section(
        &self,
        theme: &gpui_kit::component::Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_4()
            .p_5()
            .rounded(px(16.))
            .border_1()
            .border_color(theme.border)
            .child(self.section(
                self.t("settings.about"),
                self.t("settings.about_desc"),
                theme,
            ))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(self.tf(
                        "settings.version",
                        &[("v", env!("CARGO_PKG_VERSION"))],
                    ))
                    .child(self.tf(
                        "settings.data_dir",
                        &[("path", &self.paths.root().display().to_string())],
                    ))
                    .child(self.tf(
                        "settings.platform",
                        &[
                            ("os", std::env::consts::OS),
                            ("arch", std::env::consts::ARCH),
                        ],
                    )),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .child(
                        Button::new("open-data")
                            .label(self.t("settings.open_data"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.open_data_dir(cx);
                            })),
                    )
                    .child(
                        Button::new("open-logs")
                            .label(self.t("settings.open_logs"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                let dir = this.paths.logs_dir();
                                this.open_dir(dir, cx);
                            })),
                    ),
            )
    }
}
