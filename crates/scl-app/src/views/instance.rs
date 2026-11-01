//! 实例管理：内存、Java、Mod 列表。

use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::{ActiveTheme, Selectable};
use gpui_kit::*;
use scl_core::instance::Instance;

use crate::app::{LauncherApp, Page};
use crate::task::format_bytes;
use crate::ui;

const MEMORY_PRESETS: [u32; 6] = [2048, 4096, 6144, 8192, 12288, 16384];

impl LauncherApp {
    pub fn render_instance(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme().clone();
        let Some(instance) = self
            .selected_instance
            .clone()
            .and_then(|id| self.instances.get(&id).cloned())
        else {
            return div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .text_sm()
                .text_color(theme.muted_foreground)
                .child(self.t("home.empty_title"))
                .into_any_element();
        };

        let id = instance.id.clone();
        let accent = ui::accent(instance.accent_index(), &theme);
        let mods = self.list_mods(&id);

        let mut page = div()
            .flex()
            .flex_col()
            .size_full()
            .p_8()
            .gap_5()
            .id("instance-scroll")
            .overflow_y_scroll()
            .child(
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
                                    .id("instance-back")
                                    .text_sm()
                                    .text_color(theme.muted_foreground)
                                    .cursor_pointer()
                                    .child(format!("← {}", self.t("instance.back")))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.page = Page::Home;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap_3()
                                    .child(
                                        div()
                                            .size(px(38.))
                                            .rounded(px(11.))
                                            .bg(accent)
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .text_sm()
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
                                                    .text_xl()
                                                    .font_weight(FontWeight::BOLD)
                                                    .child(instance.name.clone()),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(theme.muted_foreground)
                                                    .child(instance.display_version()),
                                            ),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap_2()
                            .child(
                                Button::new("instance-play")
                                    .primary()
                                    .label(self.t("home.launch"))
                                    .on_click(cx.listener({
                                        let id = id.clone();
                                        move |this, _, _, cx| {
                                            this.play(&id, cx);
                                        }
                                    })),
                            )
                            .child(
                                Button::new("instance-delete")
                                    .label(self.t("common.delete"))
                                    .on_click(cx.listener({
                                        let id = id.clone();
                                        move |this, _, _, cx| {
                                            this.confirm_delete = Some(id.clone());
                                            cx.notify();
                                        }
                                    })),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .child(ui::badge(
                        self.tf("home.memory", &[("n", &instance.memory_mb.to_string())]),
                        theme.muted_foreground,
                        theme.secondary,
                    ))
                    .child(ui::badge(
                        instance.play_time_label(),
                        theme.muted_foreground,
                        theme.secondary,
                    ))
                    .child(ui::badge(
                        format!("{}: {}", self.t("instance.created"), instance.created_at),
                        theme.muted_foreground,
                        theme.secondary,
                    )),
            )
            .child(self.render_memory_section(&instance, &theme, cx))
            .child(self.render_instance_java_section(&instance, &theme, cx))
            .child(self.render_mods_section(&id, mods, &theme, cx))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .child(
                        Button::new("open-instance-dir")
                            .label(self.t("instance.open_dir"))
                            .on_click(cx.listener({
                                let id = id.clone();
                                move |this, _, _, cx| {
                                    let dir = this.paths.instance_dir(&id);
                                    this.open_dir(dir, cx);
                                }
                            })),
                    )
                    .child(
                        Button::new("open-mods-dir")
                            .label(self.t("instance.open_mods"))
                            .on_click(cx.listener({
                                let id = id.clone();
                                move |this, _, _, cx| {
                                    let dir = this.mods_dir(&id);
                                    this.open_dir(dir, cx);
                                }
                            })),
                    ),
            );

        page.into_any_element()
    }

    fn card(&self, theme: &gpui_kit::component::Theme, title: &'static str, desc: &'static str) -> gpui_kit::Div {
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
                            .child(desc),
                    ),
            )
    }

    fn render_memory_section(
        &self,
        instance: &Instance,
        theme: &gpui_kit::component::Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let id = instance.id.clone();
        let mut row = div().flex().flex_row().gap_2();
        for preset in MEMORY_PRESETS {
            let selected = instance.memory_mb == preset;
            row = row.child(
                Button::new(SharedString::from(format!("imem-{preset}")))
                    .label(format!("{} GB", preset / 1024))
                    .selected(selected)
                    .on_click(cx.listener({
                        let id = id.clone();
                        move |this, _, _, cx| {
                            this.update_instance(
                                &id,
                                |instance| instance.memory_mb = preset,
                                cx,
                            );
                        }
                    })),
            );
        }

        self.card(theme, self.t("instance.memory"), self.t("settings.perf_desc"))
            .child(row)
    }

    fn render_instance_java_section(
        &self,
        instance: &Instance,
        theme: &gpui_kit::component::Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let id = instance.id.clone();
        let mut list = div().flex().flex_col().gap_2();

        let auto = instance.java_path.is_none();
        list = list.child(
            div()
                .id("inst-java-auto")
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .px_3()
                .py_2()
                .rounded(px(10.))
                .bg(if auto { theme.secondary } else { theme.background })
                .cursor_pointer()
                .on_click(cx.listener({
                    let id = id.clone();
                    move |this, _, _, cx| {
                        this.update_instance(&id, |instance| instance.java_path = None, cx);
                    }
                }))
                .child(div().text_sm().child(self.t("settings.java_auto")))
                .child(if auto {
                    ui::badge(self.t("settings.in_use"), theme.green, theme.background)
                        .into_any_element()
                } else {
                    div().into_any_element()
                }),
        );

        for runtime in &self.javas {
            let path = runtime.path.to_string_lossy().to_string();
            let selected = instance.java_path.as_deref() == Some(path.as_str());
            let path_owned = path.clone();
            list = list.child(
                div()
                    .id(SharedString::from(format!("inst-java-{path}")))
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
                    .on_click(cx.listener({
                        let id = id.clone();
                        move |this, _, _, cx| {
                            let path = path_owned.clone();
                            this.update_instance(
                                &id,
                                |instance| instance.java_path = Some(path),
                                cx,
                            );
                        }
                    }))
                    .child(
                        div()
                            .text_sm()
                            .child(format!("Java {} · {}", runtime.major, runtime.vendor)),
                    )
                    .child(if selected {
                        ui::badge(self.t("settings.in_use"), theme.green, theme.background)
                            .into_any_element()
                    } else {
                        div().into_any_element()
                    }),
            );
        }

        self.card(theme, self.t("instance.java"), self.t("settings.java_desc"))
            .child(list)
    }

    fn render_mods_section(
        &self,
        instance_id: &str,
        mods: Vec<crate::app::ModEntry>,
        theme: &gpui_kit::component::Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let id = instance_id.to_string();
        let mut list = div().flex().flex_col().gap_2();

        if mods.is_empty() {
            list = list.child(
                div()
                    .px_3()
                    .py_4()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(self.t("instance.mods_empty")),
            );
        }

        for entry in mods {
            let file = entry.file_name.clone();
            let toggle_label = if entry.enabled {
                self.t("instance.mod_disable")
            } else {
                self.t("instance.mod_enable")
            };
            let size = format_bytes(entry.size);
            let name_color = if entry.enabled {
                theme.foreground
            } else {
                theme.muted_foreground
            };
            list = list.child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .px_3()
                    .py_2()
                    .rounded(px(10.))
                    .bg(theme.background)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(div().text_sm().text_color(name_color).child(file.clone()))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child(size),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap_2()
                            .child(
                                Button::new(SharedString::from(format!(
                                    "mod-toggle-{}",
                                    file
                                )))
                                .label(toggle_label)
                                .on_click(cx.listener({
                                    let id = id.clone();
                                    let file = file.clone();
                                    move |this, _, _, cx| {
                                        this.toggle_mod(&id, &file, cx);
                                    }
                                })),
                            )
                            .child(
                                Button::new(SharedString::from(format!("mod-del-{}", file)))
                                    .label(self.t("common.delete"))
                                    .on_click(cx.listener({
                                        let id = id.clone();
                                        let file = file.clone();
                                        move |this, _, _, cx| {
                                            this.delete_mod(&id, &file, cx);
                                        }
                                    })),
                            ),
                    ),
            );
        }

        self.card(theme, self.t("instance.mods"), self.t("instance.mods_desc"))
            .child(list)
    }
}
