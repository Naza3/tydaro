use super::*;

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
