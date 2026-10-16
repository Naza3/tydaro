//! Shared native visual vocabulary for the five-page desktop workspace.
use gpui_kit::{prelude::*, *};

// Component Assets embeds a small default subset; these workspace icons must
// also be present in both the native app and its headless renderer.
gpui_kit::assets::icon_assets!(
    WorkspaceIcons,
    [
        Activity,
        TrendingUp,
        Layers,
        ArrowUpDown,
        CheckCheck,
        ListFilter,
        Trash,
        Volume2,
        VolumeX,
        X,
    ]
);
pub struct AppAssets;
impl AssetSource for AppAssets {
    fn load(&self, path: &str) -> anyhow::Result<Option<std::borrow::Cow<'static, [u8]>>> {
        match WorkspaceIcons.load(path)? {
            Some(icon) => Ok(Some(icon)),
            None => gpui_kit::assets::Assets.load(path),
        }
    }
    fn list(&self, path: &str) -> anyhow::Result<Vec<SharedString>> {
        let mut paths = gpui_kit::assets::Assets.list(path)?;
        paths.extend(WorkspaceIcons.list(path)?);
        paths.sort();
        paths.dedup();
        Ok(paths)
    }
}

pub const INK: u32 = 0x233348;
pub const MUTED: u32 = 0x677688;
pub const LINE: u32 = 0xe4e9ef;
pub const TEAL: u32 = 0x008e8c;
pub const UP: u32 = 0xc92f57;
pub const DOWN: u32 = 0x007d78;
pub const BG: u32 = 0xf4f6f8;

pub fn page() -> Div {
    div()
        .size_full()
        .min_h_0()
        .flex()
        .flex_col()
        .gap_4()
        .p_5()
        .bg(rgb(BG))
        .text_size(px(14.))
        .text_color(rgb(INK))
        .overflow_hidden()
}
pub fn panel() -> Div {
    div()
        .min_w_0()
        .bg(rgb(0xffffff))
        .border_1()
        .border_color(rgb(LINE))
        .rounded_lg()
        .overflow_hidden()
}
pub fn muted(text: impl Into<SharedString>) -> Div {
    div()
        .text_size(px(12.))
        .text_color(rgb(MUTED))
        .child(text.into())
}
pub fn tag(text: impl Into<SharedString>, color: u32) -> Div {
    div()
        .px_2()
        .py_0p5()
        .rounded_sm()
        .text_size(px(11.))
        .text_color(rgb(color))
        .bg(rgb(0xedf5f5))
        .child(text.into())
}
pub fn heading(
    eyebrow: impl Into<SharedString>,
    title: impl Into<SharedString>,
    subtitle: impl Into<SharedString>,
) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .text_size(px(11.))
                .text_color(rgb(MUTED))
                .child(eyebrow.into()),
        )
        .child(
            div()
                .text_size(px(24.))
                .font_weight(FontWeight::BOLD)
                .child(title.into()),
        )
        .child(muted(subtitle))
}
