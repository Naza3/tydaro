//! Shared native visual vocabulary for the five-page desktop workspace.
use gpui_kit::*;
use tydaro_ui::{
    surface::{self, SurfaceStyle},
    tokens::LIGHT,
};

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

pub const INK: u32 = LIGHT.text;
pub const MUTED: u32 = LIGHT.muted;
pub const LINE: u32 = LIGHT.border;
pub const TEAL: u32 = LIGHT.accent;
pub const UP: u32 = LIGHT.error;
pub const DOWN: u32 = LIGHT.success;
pub const BG: u32 = LIGHT.background;

pub fn page() -> Div {
    surface::page(LIGHT)
}
pub fn panel() -> Div {
    surface::surface(LIGHT, SurfaceStyle::Panel)
}
pub fn muted(text: impl Into<SharedString>) -> Div {
    tydaro_ui::text::hint(text, LIGHT)
}
pub fn tag(text: impl Into<SharedString>, color: u32) -> Div {
    tydaro_ui::text::tag(text, color, LIGHT)
}
pub fn heading(
    eyebrow: impl Into<SharedString>,
    title: impl Into<SharedString>,
    subtitle: impl Into<SharedString>,
) -> Div {
    tydaro_ui::text::heading(eyebrow, title, subtitle, LIGHT)
}
