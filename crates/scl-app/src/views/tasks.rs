//! 任务中心：安装 / 启动进度与日志。

use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::progress::Progress;
use gpui_kit::component::ActiveTheme;
use gpui_kit::*;

use crate::app::{LauncherApp, Page, TaskStatus};
use crate::task::format_bytes;
use crate::ui;

impl LauncherApp {
    pub fn render_tasks(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme().clone();
        let running = self.running_count();
        let finished = self.tasks.iter().filter(|t| !t.is_running()).count();

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
                                    .child(self.t("tasks.title")),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(theme.muted_foreground)
                                    .child(self.tf(
                                        "tasks.summary",
                                        &[
                                            ("running", &running.to_string()),
                                            ("done", &finished.to_string()),
                                        ],
                                    )),
                            ),
                    )
                    .child(
                        Button::new("clear-tasks")
                            .label(self.t("tasks.clear"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.clear_finished_tasks();
                                cx.notify();
                            })),
                    ),
            );

        if self.tasks.is_empty() {
            page = page.child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap_3()
                    .child(div().text_3xl().child("⇣"))
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .child(self.t("tasks.empty")),
                    )
                    .child(
                        Button::new("tasks-goto-versions")
                            .primary()
                            .label(self.t("home.browse"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.page = Page::Versions;
                                cx.notify();
                            })),
                    ),
            );
            return page;
        }

        let mut list = div()
            .id("task-list")
            .flex()
            .flex_col()
            .gap_3()
            .flex_1()
            .overflow_y_scroll();

        type Row = (
            u64,
            String,
            String,
            String,
            String,
            usize,
            usize,
            u64,
            TaskStatus,
            bool,
        );
        let mut entries: Vec<Row> = Vec::new();
        for task in &self.tasks {
            entries.push((
                task.id,
                task.title.clone(),
                task.subtitle.clone(),
                task.stage.clone(),
                task.current_file.clone(),
                task.done,
                task.total,
                task.bytes,
                task.status.clone(),
                task.is_running(),
            ));
        }

        for (id, title, subtitle, stage, current, done, total, bytes, status, is_running) in entries {
            let progress = if total == 0 {
                0.0
            } else {
                (done as f32 / total as f32) * 100.0
            };
            let (status_text, status_color) = match &status {
                TaskStatus::Running => (self.t("tasks.status.running").to_string(), theme.blue),
                TaskStatus::Success => (self.t("tasks.status.success").to_string(), theme.green),
                TaskStatus::Failed(err) => (
                    self.tf("tasks.status.failed", &[("error", err)]),
                    theme.red,
                ),
                TaskStatus::Cancelled => (
                    self.t("tasks.status.cancelled").to_string(),
                    theme.muted_foreground,
                ),
            };

            let mut card = div()
                .flex()
                .flex_col()
                .gap_3()
                .p_4()
                .rounded(px(14.))
                .border_1()
                .border_color(theme.border)
                .bg(theme.background)
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_start()
                        .justify_between()
                        .gap_4()
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(2.))
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child(title),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(theme.muted_foreground)
                                        .child(subtitle),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap_3()
                                .child(div().text_xs().text_color(status_color).child(status_text))
                                .child(if is_running {
                                    Button::new(SharedString::from(format!("cancel-{id}")))
                                        .label(self.t("tasks.cancel"))
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.cancel_task(id, cx);
                                        }))
                                        .into_any_element()
                                } else {
                                    div().into_any_element()
                                }),
                        ),
                );

            if total > 0 {
                card = card
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .justify_between()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(stage.clone())
                            .child(format!("{done}/{total} · {}", format_bytes(bytes))),
                    )
                    .child(ui::bar(progress, theme.secondary, theme.primary));

                if !current.is_empty() {
                    card = card.child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(current),
                    );
                }
            } else {
                card = card.child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_3()
                        .child(
                            Progress::new(SharedString::from(format!("prog-{id}")))
                                .loading(is_running),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child(stage),
                        ),
                );
            }

            list = list.child(card);
        }

        page = page.child(list);

        if let Some(task) = self.tasks.iter().find(|t| t.is_running()) {
            let tail: Vec<String> = task.logs.iter().rev().take(8).cloned().collect();
            let mut log_box = div()
                .id("log-box")
                .h(px(160.))
                .p_3()
                .rounded(px(12.))
                .bg(theme.secondary)
                .text_xs()
                .font_family("monospace")
                .text_color(theme.muted_foreground)
                .flex()
                .flex_col()
                .gap(px(1.))
                .overflow_y_scroll();
            for line in tail.into_iter().rev() {
                log_box = log_box.child(div().child(line));
            }
            page = page.child(log_box);
        }

        page
    }
}
