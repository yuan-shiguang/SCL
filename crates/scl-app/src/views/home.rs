//! 启动台：实例列表与一键启动。

use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::{ActiveTheme, Sizable};
use gpui_kit::*;
use scl_core::instance::Instance;

use crate::app::{LauncherApp, Page};
use crate::ui;

impl LauncherApp {
    pub fn render_home(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let account = self
            .account()
            .map(|a| a.name.clone())
            .unwrap_or_else(|| "Adventurer".to_string());
        let instances: Vec<Instance> = self.instances.list().to_vec();
        let selected = self
            .selected_instance
            .clone()
            .and_then(|id| instances.iter().find(|i| i.id == id).cloned())
            .or_else(|| instances.first().cloned());

        let welcome = self.tf("home.welcome", &[("name", &account)]);

        let mut page = div()
            .flex()
            .flex_col()
            .size_full()
            .p_8()
            .gap_6()
            .id("home-scroll")
            .overflow_y_scroll()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_end()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_2xl()
                                    .font_weight(FontWeight::BOLD)
                                    .child(welcome),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(theme.muted_foreground)
                                    .child(self.t("home.subtitle")),
                            ),
                    )
                    .child(
                        Button::new("new-instance")
                            .primary()
                            .label(format!("＋ {}", self.t("home.new_instance")))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.page = Page::Versions;
                                cx.notify();
                            })),
                    ),
            );

        page = match &selected {
            Some(instance) => page.child(self.render_hero(instance, &theme, cx)),
            None => page.child(self.render_empty_state(&theme, cx)),
        };

        if !instances.is_empty() {
            page = page.child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.muted_foreground)
                    .child(self.tf("home.all_instances", &[("n", &instances.len().to_string())])),
            );

            let mut rows = div().flex().flex_col().gap_4();
            for chunk in instances.chunks(3) {
                let mut row = div().flex().flex_row().gap_4();
                for instance in chunk {
                    let is_selected = selected
                        .as_ref()
                        .map(|s| s.id == instance.id)
                        .unwrap_or(false);
                    row = row.child(self.render_instance_card(instance, is_selected, &theme, cx));
                }
                for _ in chunk.len()..3 {
                    row = row.child(div().flex_1());
                }
                rows = rows.child(row);
            }
            page = page.child(rows);
        }

        page
    }

    fn render_hero(
        &self,
        instance: &Instance,
        theme: &gpui_kit::component::Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let id = instance.id.clone();
        let running = self.running_instance.as_deref() == Some(instance.id.as_str());
        let accent = ui::accent(instance.accent_index(), theme);
        let active = self.tasks.iter().find(|t| {
            t.is_running()
                && matches!(&t.kind, crate::app::TaskKind::Run { instance } if instance == &id)
        });
        let mem = self.tf("home.memory", &[("n", &instance.memory_mb.to_string())]);

        let mut card = div()
            .w_full()
            .p_6()
            .rounded(px(18.))
            .border_1()
            .border_color(theme.border)
            .bg(theme.secondary)
            .flex()
            .flex_col()
            .gap_5()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_start()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap_3()
                                    .child(
                                        div()
                                            .size(px(44.))
                                            .rounded(px(12.))
                                            .bg(accent)
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .text_lg()
                                            .text_color(theme.primary_foreground)
                                            .font_weight(FontWeight::BOLD)
                                            .child(ui::initial(&instance.name)),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap(px(2.))
                                            .child(
                                                div()
                                                    .text_xl()
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .child(instance.name.clone()),
                                            )
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .text_color(theme.muted_foreground)
                                                    .child(instance.display_version()),
                                            ),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .gap_2()
                                    .child(ui::badge(
                                        mem,
                                        theme.muted_foreground,
                                        theme.background,
                                    ))
                                    .child(ui::badge(
                                        instance.play_time_label(),
                                        theme.muted_foreground,
                                        theme.background,
                                    )),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap_2()
                            .child(
                                Button::new("hero-manage")
                                    .label(self.t("home.manage"))
                                    .on_click(cx.listener({
                                        let id = id.clone();
                                        move |this, _, _, cx| {
                                            this.open_instance_page(&id, cx);
                                        }
                                    })),
                            )
                            .child(
                                Button::new("hero-preview")
                                    .label(self.t("home.preview"))
                                    .on_click(cx.listener({
                                        let id = id.clone();
                                        move |this, _, _, cx| {
                                            this.preview_launch(&id, cx);
                                        }
                                    })),
                            )
                            .child(
                                Button::new("hero-delete")
                                    .label(self.t("home.delete"))
                                    .on_click(cx.listener({
                                        let id = id.clone();
                                        move |this, _, _, cx| {
                                            this.confirm_delete = Some(id.clone());
                                            cx.notify();
                                        }
                                    })),
                            ),
                    ),
            );

        if let Some(task) = active {
            card = card.child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .justify_between()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(task.stage.clone())
                            .child(task.current_file.clone()),
                    )
                    .child(ui::bar(task.progress(), theme.background, accent)),
            );
        }

        card.child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap_3()
                .child(
                    Button::new("hero-play")
                        .primary()
                        .large()
                        .label(if running {
                            format!("● {}", self.t("home.running"))
                        } else {
                            format!("▶  {}", self.t("home.launch"))
                        })
                        .on_click(cx.listener({
                            let id = id.clone();
                            move |this, _, _, cx| {
                                this.play(&id, cx);
                            }
                        })),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(if running {
                            self.t("home.hint_running")
                        } else {
                            self.t("home.hint_first_launch")
                        }),
                ),
        )
    }

    fn render_empty_state(
        &self,
        theme: &gpui_kit::component::Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .w_full()
            .p_10()
            .rounded(px(18.))
            .border_1()
            .border_color(theme.border)
            .bg(theme.secondary)
            .flex()
            .flex_col()
            .items_center()
            .gap_4()
            .child(div().text_3xl().child("✦"))
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(self.t("home.empty_title")),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child(self.t("home.empty_desc")),
            )
            .child(
                Button::new("goto-versions")
                    .primary()
                    .label(self.t("home.browse"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.page = Page::Versions;
                        cx.notify();
                    })),
            )
    }

    fn render_instance_card(
        &self,
        instance: &Instance,
        selected: bool,
        theme: &gpui_kit::component::Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let accent = ui::accent(instance.accent_index(), theme);
        let id = instance.id.clone();
        let id_select = id.clone();
        let id_play = id.clone();
        let id_delete = id.clone();
        let mut card = div()
            .id((
                "instance",
                instance.accent_index() * 97 + instance.name.len(),
            ))
            .flex_1()
            .min_w(px(220.))
            .p_4()
            .rounded(px(14.))
            .border_1()
            .border_color(if selected { accent } else { theme.border })
            .bg(if selected { theme.secondary } else { theme.background })
            .flex()
            .flex_col()
            .gap_3()
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, _, cx| {
                this.selected_instance = Some(id_select.clone());
                cx.notify();
            }));

        card = card.child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_3()
                        .child(
                            div()
                                .size(px(30.))
                                .rounded(px(9.))
                                .bg(accent)
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_xs()
                                .text_color(theme.primary_foreground)
                                .font_weight(FontWeight::BOLD)
                                .child(ui::initial(&instance.name)),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::MEDIUM)
                                        .child(instance.name.clone()),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(theme.muted_foreground)
                                        .child(instance.display_version()),
                                ),
                        ),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(instance.play_time_label()),
                ),
        );

        card.child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .id(("play", instance.name.len() * 31 + instance.accent_index()))
                        .px_3()
                        .py_1()
                        .rounded(px(8.))
                        .bg(accent)
                        .text_xs()
                        .text_color(theme.primary_foreground)
                        .cursor_pointer()
                        .child(format!("▶ {}", self.t("home.launch")))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.play(&id_play, cx);
                        })),
                )
                .child(
                    div()
                        .id(("del", instance.name.len() * 17 + instance.accent_index()))
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .cursor_pointer()
                        .child(self.t("home.delete"))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.confirm_delete = Some(id_delete.clone());
                            cx.notify();
                        })),
                ),
        )
    }

    /// 生成启动命令预览（不真的启动）。
    pub fn preview_launch(&mut self, id: &str, cx: &mut Context<Self>) {
        let Some(instance) = self.instances.get(id).cloned() else {
            return;
        };
        let Some(account) = self.settings.active_account().cloned() else {
            let msg = self.t("toast.need_account").to_string();
            self.set_toast(msg, true, cx);
            return;
        };
        let title = self.t("dialog.command_title").to_string();
        let paths = self.paths.clone();
        let settings = self.settings.clone();
        let bridge = crate::task::bridge();
        let sender = bridge.sender;
        std::thread::spawn(move || {
            let result = (|| -> std::result::Result<String, String> {
                let resolved = scl_core::install::resolve_version(&paths, &instance.version)
                    .map_err(|e| e.to_string())?;
                let java = instance
                    .java_path
                    .as_ref()
                    .map(std::path::PathBuf::from)
                    .filter(|p| p.is_file())
                    .or_else(|| {
                        settings
                            .java_path
                            .as_ref()
                            .map(std::path::PathBuf::from)
                            .filter(|p| p.is_file())
                    })
                    .or_else(|| {
                        scl_core::java::find_for(resolved.required_java_major()).map(|r| r.path)
                    })
                    .ok_or_else(|| "No Java found".to_string())?;
                let options = scl_core::launch::LaunchOptions {
                    paths: &paths,
                    instance: &instance,
                    version: &resolved,
                    account: &account,
                    java: &java,
                    memory_mb: instance.memory_mb,
                    extra_jvm_args: Vec::new(),
                    extra_game_args: Vec::new(),
                };
                scl_core::launch::preview_command(&options).map_err(|e| e.to_string())
            })();
            match result {
                Ok(command) => {
                    let _ = sender.send(scl_core::download::TaskEvent::Log(format!(
                        "__preview__{command}"
                    )));
                }
                Err(err) => {
                    let _ = sender.send(scl_core::download::TaskEvent::Failed(err));
                }
            }
        });

        let mut receiver = bridge.receiver;
        cx.spawn(async move |this, cx| {
            use futures::StreamExt;
            while let Some(event) = receiver.next().await {
                let _ = this.update(cx, |app: &mut Self, cx| {
                    match event {
                        scl_core::download::TaskEvent::Log(line) => {
                            if let Some(command) = line.strip_prefix("__preview__") {
                                app.command_preview = Some((title.clone(), command.to_string()));
                            }
                        }
                        scl_core::download::TaskEvent::Failed(err) => {
                            app.set_toast(err, true, cx);
                        }
                        _ => {}
                    }
                    cx.notify();
                });
                break;
            }
        })
        .detach();
    }
}
