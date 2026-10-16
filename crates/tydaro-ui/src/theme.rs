//! Common native theme, initialized once by the application's composition root.
use gpui_kit::{
    component::{Theme, ThemeMode},
    *,
};

use crate::tokens::LIGHT;

/// Applies existing typography and interaction colors without initializing any
/// application globals, registering panels, opening windows, or reading files.
pub fn apply_light_theme(cx: &mut App) {
    Theme::change(ThemeMode::Light, None, cx);
    let theme = Theme::global_mut(cx);
    theme.font_size = px(14.);
    theme.font_family = if cfg!(target_os = "windows") {
        "Microsoft YaHei UI".into()
    } else {
        "Noto Sans CJK SC".into()
    };
    theme.primary = rgb(LIGHT.accent).into();
    theme.primary_hover = rgb(0x007c7a).into();
    theme.primary_active = rgb(0x006c6a).into();
    theme.primary_foreground = rgb(0xffffff).into();
    theme.button_primary_foreground = rgb(0xffffff).into();
    theme.accent = rgb(0xe7f5f3).into();
    theme.accent_foreground = rgb(0x007d78).into();
    theme.ring = rgb(LIGHT.accent).into();
    theme.tokens.button_primary = Hsla::from(rgb(LIGHT.accent)).into();
    theme.tokens.button_primary_hover = Hsla::from(rgb(0x007c7a)).into();
    theme.tokens.button_primary_active = Hsla::from(rgb(0x006c6a)).into();
    theme.tokens.secondary_active = Hsla::from(rgb(0xe7f5f3)).into();
}
