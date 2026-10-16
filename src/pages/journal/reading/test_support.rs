use super::super::test_support::settle;
use super::*;
use anyhow::ensure;
use gpui_kit::test::TestWindowExt;

pub(in crate::pages::journal) fn verify_rejected_notes(
    cx: &mut HeadlessAppContext,
    oversized_note: &str,
) -> anyhow::Result<()> {
    let mut handles = Vec::new();
    let result = (|| -> anyhow::Result<()> {
        let (reading, reader) = cx.update(|cx| {
            gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
                cx.new(|cx| JournalReading::new(TODAY.into(), window, cx))
            })
        })?;
        handles.push(reading);
        let (other, other_reader) = cx.update(|cx| {
            gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
                cx.new(|cx| JournalReading::new(TODAY.into(), window, cx))
            })
        })?;
        handles.push(other);
        cx.update_window(reading, |_, window, cx| {
            let note = reader.read(cx).note.clone();
            note.update(cx, |input, cx| {
                input.replace_all(oversized_note.to_owned(), window, cx)
            });
        })?;
        settle(cx, reading)?;
        cx.update_window(other, |_, window, cx| {
            let note = other_reader.read(cx).note.clone();
            note.update(cx, |input, cx| {
                input.replace_all("保留另一个阅读窗口的记录", window, cx)
            });
        })?;
        settle(cx, other)?;
        ensure!(
            !cx.update(desktop_state::save_now),
            "One reader cleared another reader's rejected input"
        );
        cx.update_window(reading, |_, window, cx| {
            for _ in 0..3 {
                ensure!(!reader.update(cx, |reader, cx| reader.request_close(window, cx)));
            }
            Ok::<_, anyhow::Error>(())
        })??;
        settle(cx, reading)?;
        cx.update_window(reading, |_, window, cx| {
            // An unscoped selector must still resolve uniquely after repeated
            // close requests; narrowing it would conceal duplicate dialogs.
            ensure!(window.has_active_dialog(cx));
            ensure!(reader.read(cx).close_confirmation_open);
            window.click("journal-reading-keep", cx);
            Ok::<_, anyhow::Error>(())
        })??;
        settle(cx, reading)?;
        cx.update_window(reading, |_, window, cx| {
            ensure!(!window.has_active_dialog(cx));
            ensure!(!reader.read(cx).close_confirmation_open);
            ensure!(reader.read(cx).note.read(cx).value().as_ref() == oversized_note);
            ensure!(!desktop_state::save_now(cx));
            for _ in 0..3 {
                ensure!(!reader.update(cx, |reader, cx| reader.request_close(window, cx)));
            }
            Ok::<_, anyhow::Error>(())
        })??;
        settle(cx, reading)?;
        cx.update_window(reading, |_, window, cx| window.press("escape", cx))?;
        settle(cx, reading)?;
        cx.update_window(reading, |_, window, cx| {
            ensure!(!window.has_active_dialog(cx));
            ensure!(!reader.read(cx).close_confirmation_open);
            ensure!(reader.read(cx).note.read(cx).value().as_ref() == oversized_note);
            ensure!(!desktop_state::save_now(cx));
            for _ in 0..3 {
                ensure!(!reader.update(cx, |reader, cx| reader.request_close(window, cx)));
            }
            Ok::<_, anyhow::Error>(())
        })??;
        settle(cx, reading)?;
        cx.update_window(reading, |_, window, cx| {
            window.click("journal-reading-discard", cx)
        })?;
        cx.run_until_parked();
        ensure!(!cx.update(|cx| reader.read(cx).close_confirmation_open));
        ensure!(cx.update_window(reading, |_, _, _| ()).is_err());
        ensure!(cx.update(desktop_state::save_now));
        ensure!(
            cx.update(|cx| cx.global::<DesktopState>().journal.days[TODAY].note
                == "保留另一个阅读窗口的记录")
        );
        Ok(())
    })();
    for handle in handles {
        let _ = cx.update_window(handle, |_, window, _| window.remove_window());
    }
    result
}
