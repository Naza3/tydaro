//! Actual native pointer/input regression for the experimental foundations.
use crate::{
    component_gallery::{self, ComponentGallery},
    ui_foundation::{Family, Variant},
};
use anyhow::{Result, ensure};
use gpui_kit::{
    AnyWindowHandle, Entity, HeadlessAppContext, ScrollDelta, point, px, size, test::TestWindowExt,
};
use std::{path::Path, sync::Arc};

fn render(cx: &mut HeadlessAppContext, handle: AnyWindowHandle) -> Result<()> {
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))?;
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))?;
    Ok(())
}
fn click(cx: &mut HeadlessAppContext, handle: AnyWindowHandle, id: &str) -> Result<()> {
    for _ in 0..12 {
        let visible = cx.update_window(handle, |_, window, cx| {
            let node = window.find(id.to_owned());
            if node.visible() && node.bounds().bottom() < window.viewport_size().height {
                return true;
            }
            window.scroll(
                "gallery-content-scroll",
                ScrollDelta::Lines(point(0., -3.)),
                cx,
            );
            false
        })?;
        if visible {
            break;
        }
        render(cx, handle)?;
    }
    cx.update_window(handle, |_, window, cx| window.click(id.to_owned(), cx))?;
    render(cx, handle)
}
fn input(
    cx: &mut HeadlessAppContext,
    handle: AnyWindowHandle,
    id: &str,
    value: &str,
) -> Result<()> {
    click(cx, handle, id)?;
    cx.update_window(handle, |_, window, cx| {
        window.press("ctrl-a", cx);
        window.press("backspace", cx);
        window.input(value, cx);
    })?;
    render(cx, handle)
}
fn family(cx: &mut HeadlessAppContext, handle: AnyWindowHandle, item: Family) -> Result<()> {
    click(cx, handle, &format!("gallery-family-{}", item.key()))
}
fn capture(
    cx: &mut HeadlessAppContext,
    handle: AnyWindowHandle,
    path: &Path,
    suffix: &str,
) -> Result<()> {
    // GPUI 0.3.9 has no Windows PlatformHeadlessRenderer. Native input/layout
    // checks still run; Windows appearance is reviewed in a real desktop window.
    #[cfg(target_os = "windows")]
    {
        let _ = (cx, handle, path, suffix);
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        let stem = path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("gallery");
        cx.capture_screenshot(handle)?
            .save(path.with_file_name(format!("{stem}-{suffix}.png")))?;
        Ok(())
    }
}
pub fn run(path: &str) -> Result<()> {
    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(crate::desktop_ui::AppAssets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(|cx| {
        crate::workspace::init(cx, false);
        // Match GPUI Kit's dialog tests: pointer targets must have settled
        // bounds rather than the first frame of a 250 ms entrance animation.
        cx.set_reduce_motion(true);
    });
    let (handle, gallery) = cx.update(component_gallery::open)?;
    let output = Path::new(path);
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    render(&mut cx, handle)?;
    #[cfg(target_os = "windows")]
    println!(
        "Windows: testing native layout/input; headless screenshots unavailable in the pinned GPUI. Review actual desktop captures separately."
    );
    let result = verify(&mut cx, handle, &gallery, output);
    if result.is_err() {
        let _ = capture(&mut cx, handle, output, "failure");
    }
    result
}
fn verify(
    cx: &mut HeadlessAppContext,
    handle: AnyWindowHandle,
    gallery: &Entity<ComponentGallery>,
    output: &Path,
) -> Result<()> {
    for (width, height) in [(1536, 960), (1280, 800)] {
        cx.update_window(handle, |_, window, cx| {
            window.resize(size(px(width as f32), px(height as f32)));
            window.bounds_changed(cx);
        })?;
        render(cx, handle)?;
        for item in Family::ALL {
            family(cx, handle, item)?;
            ensure!(cx.update(|cx| gallery.read(cx).selected_family()) == item);
            capture(cx, handle, output, &format!("{}-{width}", item.key()))?;
        }
    }
    cx.update_window(handle, |_, window, cx| {
        window.resize(size(px(1536.), px(960.)));
        window.bounds_changed(cx);
    })?;
    family(cx, handle, Family::Actions)?;
    for variant in Variant::ALL {
        click(cx, handle, &format!("gallery-{}-action-run", variant.key()))?;
        ensure!(cx.update(|cx| gallery.read(cx).controls_snapshot(variant, cx).2));
    }
    click(cx, handle, "gallery-c-action-undo")?;
    ensure!(!cx.update(|cx| gallery.read(cx).controls_snapshot(Variant::Context, cx).2));
    family(cx, handle, Family::Input)?;
    input(cx, handle, "gallery-a-name-input", "")?;
    click(cx, handle, "gallery-a-name-save")?;
    ensure!(
        cx.update(|cx| gallery.read(cx).controls_snapshot(Variant::Precision, cx).0) == "重点观察"
    );
    input(cx, handle, "gallery-a-name-input", "研究清单")?;
    click(cx, handle, "gallery-a-name-save")?;
    click(cx, handle, "gallery-c-name-edit")?;
    input(cx, handle, "gallery-c-name-input", "不应保存")?;
    click(cx, handle, "gallery-c-name-cancel")?;
    ensure!(
        cx.update_window(handle, |_, window, cx| gallery.read(cx).controls_focus(
            Variant::Context,
            window,
            cx
        ))? == (true, false, false),
        "Hidden C name input retained keyboard focus"
    );
    ensure!(
        cx.update(|cx| gallery.read(cx).controls_snapshot(Variant::Context, cx).0) == "重点观察"
    );
    family(cx, handle, Family::Number)?;
    input(cx, handle, "gallery-a-number-input", "NaN")?;
    click(cx, handle, "gallery-a-number-apply")?;
    ensure!(cx.update(|cx| gallery.read(cx).controls_snapshot(Variant::Precision, cx).1) == 2.);
    click(cx, handle, "gallery-b-number-plus")?;
    ensure!(cx.update(|cx| gallery.read(cx).controls_snapshot(Variant::Calm, cx).1) == 2.5);
    click(cx, handle, "gallery-c-number-tick-5")?;
    ensure!(cx.update(|cx| gallery.read(cx).controls_snapshot(Variant::Context, cx).1) == 5.);
    family(cx, handle, Family::Choice)?;
    for variant in Variant::ALL {
        if variant == Variant::Context {
            click(cx, handle, "gallery-c-choice-trigger")?;
        }
        click(cx, handle, &format!("gallery-{}-choice-1", variant.key()))?;
        ensure!(cx.update(|cx| gallery.read(cx).selection_snapshot(variant, cx).0) == 1);
    }
    family(cx, handle, Family::Toggle)?;
    for variant in Variant::ALL {
        click(cx, handle, &format!("gallery-{}-toggle", variant.key()))?;
        ensure!(!cx.update(|cx| gallery.read(cx).selection_snapshot(variant, cx).1));
    }
    family(cx, handle, Family::Tabs)?;
    for variant in Variant::ALL {
        click(cx, handle, &format!("gallery-{}-tab-2", variant.key()))?;
        ensure!(cx.update(|cx| gallery.read(cx).selection_snapshot(variant, cx).2) == 2);
    }
    family(cx, handle, Family::Tokens)?;
    for variant in Variant::ALL {
        click(cx, handle, &format!("gallery-{}-filter-0", variant.key()))?;
        ensure!(!cx.update(|cx| gallery.read(cx).selection_snapshot(variant, cx).3[0]));
    }
    family(cx, handle, Family::List)?;
    for variant in Variant::ALL {
        click(cx, handle, &format!("gallery-{}-list-row-1", variant.key()))?;
        ensure!(cx.update(|cx| gallery.read(cx).display_snapshot(variant, cx).0) == 1);
    }
    family(cx, handle, Family::Disclosure)?;
    for variant in Variant::ALL {
        click(
            cx,
            handle,
            &format!("gallery-{}-disclosure-toggle", variant.key()),
        )?;
        ensure!(cx.update(|cx| gallery.read(cx).display_snapshot(variant, cx).1));
    }
    family(cx, handle, Family::Feedback)?;
    click(cx, handle, "gallery-a-feedback-run")?;
    ensure!(cx.update(|cx| gallery.read(cx).display_snapshot(Variant::Precision, cx).2) == 3);
    click(cx, handle, "gallery-a-feedback-retry")?;
    ensure!(cx.update(|cx| gallery.read(cx).display_snapshot(Variant::Precision, cx).2) == 0);
    click(cx, handle, "gallery-b-feedback-run")?;
    click(cx, handle, "gallery-b-feedback-undo")?;
    ensure!(cx.update(|cx| gallery.read(cx).display_snapshot(Variant::Calm, cx).2) == 3);
    click(cx, handle, "gallery-c-feedback-run")?;
    click(cx, handle, "gallery-c-feedback-cancel")?;
    ensure!(cx.update(|cx| gallery.read(cx).display_snapshot(Variant::Context, cx).2) == 3);
    ensure!(
        cx.update_window(handle, |_, window, _| window
            .try_find("gallery-c-feedback-result")
            .is_some())?,
        "Cancel must execute its callback and show a receipt"
    );
    click(cx, handle, "gallery-c-feedback-run")?;
    click(cx, handle, "gallery-c-feedback-confirm")?;
    ensure!(cx.update(|cx| gallery.read(cx).display_snapshot(Variant::Context, cx).2) == 0);
    family(cx, handle, Family::Input)?;
    ensure!(
        cx.update(|cx| gallery.read(cx).controls_snapshot(Variant::Precision, cx).0) == "研究清单"
    );
    click(cx, handle, "gallery-prefer-b")?;
    ensure!(cx.update(|cx| gallery.read(cx).preference(Family::Input, cx)) == Some(Variant::Calm));
    click(cx, handle, "gallery-copy-preferences")?;
    ensure!(cx.update(|cx| {
        cx.read_from_clipboard()
            .and_then(|item| item.text())
            .is_some_and(|text| text.contains("文本输入") && text.contains('B'))
    }));
    family(cx, handle, Family::List)?;
    click(cx, handle, "gallery-solo-c")?;
    capture(cx, handle, output, "solo-c")?;
    click(cx, handle, "gallery-compare")?;
    click(cx, handle, "gallery-clear-preferences")?;
    ensure!(
        cx.update(|cx| gallery.read(cx).preference(Family::Input, cx))
            .is_none()
    );
    verify_integration(cx)?;
    println!(
        "PASS: 10 native foundation categories × 3 variants at two sizes; actual input, selection, cancellation, retry, undo and confirmation; category state retention, local preferences and desktop integration"
    );
    Ok(())
}

fn verify_integration(cx: &mut HeadlessAppContext) -> Result<()> {
    let workspace = cx.update(|cx| crate::workspace::open_workspace(0, None, cx))?;
    let handle = cx.update(|cx| workspace.read(cx).handle);
    render(cx, handle)?;
    let before = cx.update(|cx| workspace.read(cx).dock.read(cx).dump(cx));
    let active = cx.update(|cx| {
        cx.global::<crate::desktop_state::DesktopState>()
            .active_page
    });
    click(cx, handle, "desktop-gallery")?;
    family(cx, handle, Family::Input)?;
    click(cx, handle, "desktop-nav-market")?;
    ensure!(
        cx.update(|cx| workspace.read(cx).dock.read(cx).dump(cx)) == before,
        "Gallery changed the market dock"
    );
    ensure!(
        cx.update(|cx| cx
            .global::<crate::desktop_state::DesktopState>()
            .active_page)
            == active
    );
    click(cx, handle, "desktop-gallery")?;
    ensure!(
        cx.update_window(handle, |_, window, _| window
            .find("gallery-a-name-input")
            .visible())?,
        "Reopening gallery lost its category"
    );
    click(cx, handle, "gallery-a-name-input")?;
    let editor_focus = cx.update_window(handle, |_, window, cx| window.focused(cx))?;
    ensure!(editor_focus.is_some(), "Gallery editor did not take focus");
    click(cx, handle, "desktop-notifications")?;
    ensure!(
        cx.update_window(handle, |_, window, _| !editor_focus
            .as_ref()
            .unwrap()
            .is_focused(window))?,
        "Leaving Gallery via notifications kept its hidden editor focused"
    );
    ensure!(
        cx.update(|cx| cx
            .global::<crate::desktop_state::DesktopState>()
            .active_page)
            == crate::desktop_state::PageId::Watch
    );
    click(cx, handle, "desktop-nav-market")?;
    Ok(())
}
