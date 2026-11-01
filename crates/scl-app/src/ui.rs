//! Small shared rendering helpers.

use gpui_kit::*;

/// First character of a name, uppercased — used by avatar bubbles.
pub fn initial(name: &str) -> String {
    name.chars()
        .next()
        .map(|c| c.to_uppercase().to_string())
        .unwrap_or_else(|| "?".to_string())
}

/// Stable accent colour per instance, so tiles look varied but consistent.
pub fn accent(index: usize, theme: &gpui_kit::component::Theme) -> Hsla {
    match index % 6 {
        0 => theme.blue,
        1 => theme.green,
        2 => theme.yellow,
        3 => theme.red,
        4 => theme.magenta,
        _ => theme.cyan,
    }
}

pub fn badge(text: impl Into<SharedString>, fg: Hsla, bg: Hsla) -> impl IntoElement {
    div()
        .px_2()
        .py(px(1.))
        .rounded(px(6.))
        .bg(bg)
        .text_xs()
        .text_color(fg)
        .child(text.into())
}

/// Slim progress bar drawn from two divs (no animation needed).
pub fn bar(value: f32, track: Hsla, fill: Hsla) -> impl IntoElement {
    let value = value.clamp(0.0, 100.0);
    div()
        .h(px(6.))
        .w_full()
        .rounded(px(3.))
        .bg(track)
        .child(
            div()
                .h_full()
                .w(relative(value / 100.0))
                .rounded(px(3.))
                .bg(fill),
        )
}
