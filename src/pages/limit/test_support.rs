use super::*;

/// Regression using production native inputs and commands; called by architecture smoke.
pub(crate) fn verify_rejected_edits(
    cx: &mut HeadlessAppContext,
    handle: AnyWindowHandle,
) -> anyhow::Result<()> {
    use anyhow::ensure;
    cx.update_window(handle, |_, window, cx| {
        let page = cx.new(|cx| LimitPage::new(window, cx));
        page.update(cx, |page, cx| -> anyhow::Result<()> {
            let original = page.rows.clone();
            let row = page
                .rows
                .first()
                .ok_or_else(|| anyhow::anyhow!("Research regression needs a fixture row"))?
                .clone();
            let id = row.id.clone();
            page.begin_edit(row, window, cx);
            let oversized = "x".repeat(64 * 1024 + 1);
            page.edit_reason.update(cx, |input, cx| {
                input.set_value(oversized.clone(), window, cx)
            });
            let before = cx.global::<DesktopState>().research.clone();
            page.confirm_edit(cx);
            ensure!(
                page.editing.is_some() && page.error.is_some(),
                "Rejected research save closed the editor or cleared its error"
            );
            ensure!(
                page.edit_reason.read(cx).value().as_str() == oversized,
                "Rejected research save discarded original input"
            );
            ensure!(
                cx.global::<DesktopState>().research == before,
                "Rejected research save changed domain data"
            );
            ensure!(
                serde_json::to_value(&page.rows)? == serde_json::to_value(&original)?,
                "Rejected research save replaced displayed rows"
            );
            ensure!(
                !desktop_state::save_now(cx),
                "Unaccepted research text did not block close"
            );
            page.edit_reason
                .update(cx, |input, cx| input.set_value("修正后原因", window, cx));
            page.confirm_edit(cx);
            ensure!(
                page.editing.is_none() && page.error.is_none(),
                "Corrected research save did not finish"
            );
            ensure!(
                page.rows.iter().find(|r| r.id == id).unwrap().reason == "修正后原因",
                "Corrected research input was not committed"
            );
            ensure!(
                desktop_state::save_now(cx),
                "Successful research save left a stale close guard"
            );

            let mut deleted = original[0].clone();
            deleted.id = "regression-undo-row".into();
            deleted.date = "2026-10-15".into();
            deleted.reason = oversized.clone();
            page.deleted = vec![deleted];
            let stable = serde_json::to_value(&page.rows)?;
            page.undo_delete(cx);
            ensure!(
                page.deleted.len() == 1 && page.deleted[0].reason == oversized,
                "Rejected undo consumed the recovery cache"
            );
            ensure!(
                serde_json::to_value(&page.rows)? == stable && page.error.is_some(),
                "Rejected undo mutated rows or hid failure"
            );
            page.deleted[0].reason = "可重试恢复".into();
            page.undo_delete(cx);
            ensure!(
                page.deleted.is_empty() && page.rows.iter().any(|r| r.id == "regression-undo-row"),
                "Successful retry failed to consume undo cache"
            );

            let row = page.rows.iter().find(|r| r.id == id).unwrap().clone();
            page.begin_edit(row, window, cx);
            page.edit_reason
                .update(cx, |input, cx| input.set_value(oversized, window, cx));
            page.confirm_edit(cx);
            ensure!(
                !desktop_state::save_now(cx),
                "Rejected second edit did not retain close guard"
            );
            page.cancel_edit(cx);
            ensure!(
                desktop_state::save_now(cx),
                "Explicit research cancel left close guard"
            );
            ensure!(
                page.persist(original, cx),
                "Could not restore regression research rows"
            );
            Ok(())
        })
    })??;
    println!(
        "PASS: oversized research edits retain input/errors; correction and cancellation work; failed undo preserves its cache"
    );
    Ok(())
}
