//! Composition root: only this layer knows the shell, workspaces and services.
use gpui_kit::{
    component::{
        WindowExt,
        button::{Button, ButtonVariants},
    },
    prelude::*,
    *,
};

use crate::{app_runtime, desktop_state, gallery_preferences, workspace};

#[derive(Default)]
struct QuitConfirmation {
    open: bool,
}
impl Global for QuitConfirmation {}

fn init_ui(cx: &mut App) {
    gpui_kit::init(cx);
    gpui_kit::component::set_locale("zh-CN");
    tydaro_ui::theme::apply_light_theme(cx);
}

/// Tests use the same composition root without personal data or background ticks.
pub(crate) fn init(cx: &mut App, persistent: bool) {
    init_ui(cx);
    cx.set_global(QuitConfirmation::default());
    desktop_state::init(persistent, cx);
    crate::contents::init_broker(cx);
    crate::market_data::init_mock(cx);
    gallery_preferences::init(persistent, cx);
    app_runtime::init(cx);
    workspace::init(
        cx,
        persistent,
        workspace::WorkspaceHost {
            build_main: |dock, window, cx| {
                let boards = cx.new(|cx| crate::board_bar::BoardBar::new(window, cx));
                cx.new(|cx| crate::desktop::Desktop::new(dock, Some(boards), window, cx))
                    .into()
            },
            close_main: request_close,
            market_visible: |cx| {
                cx.global::<desktop_state::DesktopState>().active_page
                    == desktop_state::PageId::Market
            },
        },
    );
}

fn finish_quit(cx: &mut App) {
    app_runtime::stop(cx);
    cx.quit();
}

fn request_close(window: &mut Window, cx: &mut App) -> bool {
    if cx.global::<QuitConfirmation>().open {
        return false;
    }
    let desktop_saved = desktop_state::save_now(cx);
    let layout_result = workspace::flush(cx);
    if desktop_saved && layout_result.is_ok() {
        finish_quit(cx);
        return true;
    }
    let mut errors = Vec::new();
    if !desktop_saved {
        errors.push(
            cx.global::<desktop_state::DesktopState>()
                .status_message()
                .to_owned(),
        );
    }
    if let Err(error) = layout_result {
        errors.push(format!("布局保存失败：{error}"));
    }
    let detail = errors.join("\n");
    cx.global_mut::<QuitConfirmation>().open = true;
    window.open_dialog(cx, move |dialog, _, _| {
        dialog
            .title("部分修改尚未保存")
            .width(px(480.))
            .on_close(|_, _, cx| {
                cx.global_mut::<QuitConfirmation>().open = false;
            })
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(detail.clone())
                    .child("继续编辑可保留内存中的修改。仍然退出将丢弃尚未保存的修改。")
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .gap_2()
                            .child(
                                Button::new("desktop-quit-cancel")
                                    .label("继续编辑")
                                    .primary()
                                    .on_click(|_, window, cx| {
                                        cx.global_mut::<QuitConfirmation>().open = false;
                                        window.close_dialog(cx);
                                    }),
                            )
                            .child(
                                Button::new("desktop-quit-discard")
                                    .label("仍然退出")
                                    .on_click(|_, _, cx| finish_quit(cx)),
                            ),
                    ),
            )
    });
    false
}

pub(crate) fn run() {
    gpui_kit::application()
        .with_assets(crate::desktop_ui::AppAssets)
        .run(|cx| {
            init(cx, true);
            if let Err(error) = workspace::open_initial(cx) {
                eprintln!("Cannot open Tydaro: {error:#}");
                cx.quit();
                return;
            }
            app_runtime::start(cx);
            cx.activate(true);
        });
}

/// The design tool installs UI and its own preferences, never business storage.
pub(crate) fn run_gallery() {
    gpui_kit::application()
        .with_assets(crate::desktop_ui::AppAssets)
        .run(|cx| {
            init_ui(cx);
            gallery_preferences::init(true, cx);
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();
            if let Err(error) = crate::component_gallery::open(cx) {
                eprintln!("Cannot open component gallery: {error:#}");
                cx.quit();
                return;
            }
            cx.activate(true);
        });
}

#[cfg(feature = "test-support")]
pub(crate) fn smoke_close_confirmation(
    cx: &mut HeadlessAppContext,
    handle: AnyWindowHandle,
) -> anyhow::Result<()> {
    use anyhow::ensure;
    use gpui_kit::test::TestWindowExt;

    fn settle(cx: &mut HeadlessAppContext, handle: AnyWindowHandle) -> anyhow::Result<()> {
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| window.render_frame(cx))?;
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| window.render_frame(cx))?;
        Ok(())
    }

    const KEY: &str = "quit-confirmation-regression";
    cx.update(|cx| {
        cx.set_reduce_motion(true);
        desktop_state::report_pending_input(cx, KEY, Some("待修正的测试输入".into()));
    });
    for cancel_with_escape in [false, true] {
        cx.update_window(handle, |_, window, cx| {
            ensure!(!request_close(window, cx));
            ensure!(!request_close(window, cx));
            Ok::<_, anyhow::Error>(())
        })??;
        settle(cx, handle)?;
        cx.update_window(handle, |_, window, cx| {
            // find rejects ambiguous IDs, so repeated requests must leave one dialog.
            ensure!(window.find("desktop-quit-cancel").visible());
            ensure!(window.find("desktop-quit-discard").visible());
            if cancel_with_escape {
                window.press("escape", cx);
            } else {
                window.click("desktop-quit-cancel", cx);
            }
            Ok::<_, anyhow::Error>(())
        })??;
        settle(cx, handle)?;
        cx.update_window(handle, |_, window, cx| {
            ensure!(!window.has_active_dialog(cx));
            ensure!(!cx.global::<QuitConfirmation>().open);
            ensure!(!desktop_state::save_now(cx));
            Ok::<_, anyhow::Error>(())
        })??;
    }
    cx.update(|cx| desktop_state::report_pending_input(cx, KEY, None));
    ensure!(cx.update(desktop_state::save_now));
    println!(
        "PASS: repeated main-window close requests share one confirmation; cancel and Escape preserve pending edits and permit reopening"
    );
    Ok(())
}
