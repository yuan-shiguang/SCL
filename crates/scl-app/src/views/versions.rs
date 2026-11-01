//! 版本库：浏览 Mojang 版本清单并安装。

use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::Input;
use gpui_kit::component::{ActiveTheme, Selectable};
use gpui_kit::*;
use scl_core::meta::VersionSummary;

use crate::app::{LauncherApp, Page, VersionFilter};
use crate::ui;

impl LauncherApp {
    pub fn render_versions(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme().clone();
        let query = self.search.read(cx).value().to_lowercase();
        let installed = self.installed_versions();
        let filter = self.filter;

        let filtered: Vec<VersionSummary> = self
            .manifest
            .as_ref()
            .map(|manifest| {
                manifest
                    .versions
                    .iter()
                    .filter(|v| match filter {
                        VersionFilter::Release => v.is_release(),
                        VersionFilter::Snapshot => v.version_type == "snapshot",
                        VersionFilter::All => {
                            v.version_type != "old_beta" && v.version_type != "old_alpha"
                        }
                    })
                    .filter(|v| query.is_empty() || v.id.to_lowercase().contains(&query))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();

        let total = filtered.len();
        let shown = self.shown.min(total);
        let visible: Vec<VersionSummary> = filtered.into_iter().take(shown).collect();

        let mut filter_row = div().flex().flex_row().items_center().gap_2();
        for (index, option) in [
            VersionFilter::Release,
            VersionFilter::Snapshot,
            VersionFilter::All,
        ]
        .into_iter()
        .enumerate()
        {
            let selected = self.filter == option;
            filter_row = filter_row.child(
                Button::new(SharedString::from(format!("filter-{index}")))
                    .label(self.t(option.key()))
                    .selected(selected)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.filter = option;
                        this.shown = 40;
                        cx.notify();
                    })),
            );
        }

        let mut page = div()
            .flex()
            .flex_col()
            .size_full()
            .p_8()
            .gap_5()
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
                                    .child(self.t("versions.title")),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(theme.muted_foreground)
                                    .child(self.t("versions.subtitle")),
                            ),
                    )
                    .child(
                        Button::new("refresh-manifest")
                            .label(if self.manifest_loading {
                                self.t("versions.refreshing")
                            } else {
                                self.t("versions.refresh")
                            })
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.refresh_manifest(cx);
                            })),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_3()
                    .child(div().flex_1().child(Input::new(&self.search).cleanable(true)))
                    .child(filter_row),
            );

        if let Some(error) = self.manifest_error.clone() {
            page = page.child(
                div()
                    .p_4()
                    .rounded(px(12.))
                    .border_1()
                    .border_color(theme.red)
                    .text_sm()
                    .text_color(theme.red)
                    .child(self.tf("versions.error", &[("error", &error)])),
            );
        }

        if self.manifest.is_none() && self.manifest_error.is_none() {
            page = page.child(
                div()
                    .p_8()
                    .rounded(px(14.))
                    .border_1()
                    .border_color(theme.border)
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child(self.t("versions.fetching")),
            );
        }

        let mut list = div()
            .id("version-list")
            .flex()
            .flex_col()
            .gap_2()
            .flex_1()
            .overflow_y_scroll();

        for (index, version) in visible.iter().enumerate() {
            let is_installed = installed.iter().any(|v| v == &version.id || v.starts_with(&version.id));
            let accent = ui::accent(index, &theme);
            let version_id = version.id.clone();

            let row = div()
                .id(SharedString::from(format!("row-{}", version.id)))
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .px_4()
                .py_3()
                .rounded(px(12.))
                .border_1()
                .border_color(theme.border)
                .bg(theme.background)
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_3()
                        .child(
                            div()
                                .w(px(6.))
                                .h(px(28.))
                                .rounded(px(3.))
                                .bg(if is_installed { accent } else { theme.border }),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::MEDIUM)
                                        .child(version.id.clone()),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(theme.muted_foreground)
                                        .child(format!(
                                            "{} · {}",
                                            self.t(type_key(&version.version_type)),
                                            version.release_date()
                                        )),
                                ),
                        ),
                );

            let action = if is_installed {
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_3()
                    .child(ui::badge(
                        self.t("versions.installed"),
                        theme.green,
                        theme.secondary,
                    ))
                    .child(
                        Button::new(SharedString::from(format!("open-{}", version.id)))
                            .label(self.t("versions.open"))
                            .on_click(cx.listener({
                                let version_id = version_id.clone();
                                move |this, _, _, cx| {
                                    this.page = Page::Home;
                                    this.selected_instance = this
                                        .instances
                                        .list()
                                        .iter()
                                        .find(|i| i.version.starts_with(&version_id))
                                        .map(|i| i.id.clone());
                                    cx.notify();
                                }
                            })),
                    )
                    .into_any_element()
            } else {
                Button::new(SharedString::from(format!("install-{}", version.id)))
                    .primary()
                    .label(self.t("versions.install"))
                    .on_click(cx.listener({
                        let version_id = version_id.clone();
                        move |this, _, _, cx| {
                            this.open_install_dialog(version_id.clone(), cx);
                        }
                    }))
                    .into_any_element()
            };

            list = list.child(row.child(action));
        }

        page = page.child(list);

        if shown < total {
            page = page.child(div().flex().justify_center().child(
                Button::new("load-more")
                    .label(self.tf(
                        "versions.load_more",
                        &[("shown", &shown.to_string()), ("total", &total.to_string())],
                    ))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.shown += 60;
                        cx.notify();
                    })),
            ));
        }

        page
    }
}

fn type_key(kind: &str) -> &'static str {
    match kind {
        "release" => "versions.type.release",
        "snapshot" => "versions.type.snapshot",
        "old_beta" => "versions.type.old_beta",
        "old_alpha" => "versions.type.old_alpha",
        _ => "versions.type.other",
    }
}
