use super::*;
use gpui_kit::test::TestWindowExt;

pub(super) fn settle(cx: &mut HeadlessAppContext, handle: AnyWindowHandle) -> anyhow::Result<()> {
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))?;
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))?;
    Ok(())
}

/// Exercise rejected input through production native entities and buttons. A
/// command rejection must not discard the only copy of a user's text.
pub(crate) fn smoke_rejected_inputs(cx: &mut HeadlessAppContext) -> anyhow::Result<()> {
    use anyhow::ensure;

    // This helper also runs from the full smoke entry point, which otherwise
    // animates dialogs. Test native clicks against stable hit-test bounds.
    cx.update(|cx| cx.set_reduce_motion(true));

    let mut handles = Vec::new();
    let result = (|| -> anyhow::Result<()> {
        let (main, page) = cx.update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                        point(px(20.), px(20.)),
                        size(px(1536.), px(960.)),
                    ))),
                    ..Default::default()
                },
                cx,
                |window, cx| cx.new(|cx| JournalPage::new(window, cx)),
            )
        })?;
        handles.push(main);
        cx.update_window(main, |_, window, cx| {
            page.update(cx, |page, cx| page.create(window, cx))
        })?;
        let oversized_draft = "x".repeat(64 * 1024 + 1);
        cx.update_window(main, |_, window, cx| {
            let basis = page.read(cx).editor.as_ref().unwrap().basis.clone();
            basis.update(cx, |input, cx| {
                input.replace_all(oversized_draft.clone(), window, cx)
            });
        })?;
        settle(cx, main)?;
        for button in ["journal-draft-save", "journal-editor-close"] {
            cx.update_window(main, |_, window, cx| window.click(button, cx))?;
            settle(cx, main)?;
            cx.update(|cx| {
                let page = page.read(cx);
                ensure!(page.editor.is_some() && page.error.is_some());
                ensure!(
                    page.editor
                        .as_ref()
                        .unwrap()
                        .basis
                        .read(cx)
                        .value()
                        .as_ref()
                        == oversized_draft
                );
                ensure!(
                    !desktop_state::save_now(cx),
                    "Rejected draft did not protect application close"
                );
                Ok::<_, anyhow::Error>(())
            })?;
        }
        cx.update_window(main, |_, window, cx| {
            page.update(cx, |page, cx| page.select_date("2026-10-15", window, cx));
            ensure!(page.read(cx).date == TODAY && page.read(cx).editor.is_some());
            let basis = page.read(cx).editor.as_ref().unwrap().basis.clone();
            basis.update(cx, |input, cx| {
                input.replace_all("已修正的研究依据", window, cx)
            });
            Ok::<_, anyhow::Error>(())
        })??;
        settle(cx, main)?;
        cx.update_window(main, |_, window, cx| window.click("journal-draft-save", cx))?;
        settle(cx, main)?;
        ensure!(cx.update(|cx| page.read(cx).editor.is_none()));

        let oversized_note = "n".repeat(256 * 1024 + 1);
        cx.update_window(main, |_, window, cx| {
            let note = page.read(cx).note.clone();
            note.update(cx, |input, cx| {
                input.replace_all(oversized_note.clone(), window, cx)
            });
        })?;
        settle(cx, main)?;
        cx.update(|cx| {
            ensure!(page.read(cx).note_error.is_some());
            ensure!(page.read(cx).note.read(cx).value().as_ref() == oversized_note);
            desktop_state::dispatch(
                cx,
                Command::SetNote {
                    date: TODAY.into(),
                    text: "另一个窗口的有效笔记".into(),
                },
            )?;
            Ok::<_, anyhow::Error>(())
        })?;
        settle(cx, main)?;
        cx.update_window(main, |_, window, cx| {
            page.update(cx, |page, cx| page.select_date("2026-10-15", window, cx));
            ensure!(page.read(cx).date == TODAY);
            ensure!(page.read(cx).note.read(cx).value().as_ref() == oversized_note);
            let note = page.read(cx).note.clone();
            note.update(cx, |input, cx| {
                input.replace_all("修正后的笔记", window, cx)
            });
            Ok::<_, anyhow::Error>(())
        })??;
        settle(cx, main)?;
        ensure!(cx.update(desktop_state::save_now));

        reading::verify_rejected_notes(cx, &oversized_note)?;
        println!(
            "PASS: rejected drafts/notes retain native input, block destructive close/date changes, and recover without losing another reader's data"
        );
        Ok(())
    })();
    for handle in handles {
        let _ = cx.update_window(handle, |_, window, _| window.remove_window());
    }
    result
}
