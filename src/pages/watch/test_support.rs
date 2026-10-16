use super::*;

/// Regression using production native inputs and commands; called by architecture smoke.
pub(crate) fn verify_rejected_edits(
    cx: &mut HeadlessAppContext,
    handle: AnyWindowHandle,
) -> anyhow::Result<()> {
    use anyhow::ensure;
    cx.update_window(handle, |_, window, cx| {
        let page = cx.new(|cx| WatchPage::new(window, cx));
        let original_rules = cx.global::<DesktopState>().rules.clone();
        page.update(cx, |page, cx| -> anyhow::Result<()> {
            page.begin_group(None, window, cx);
            let invalid = "组".repeat(25);
            page.name
                .update(cx, |input, cx| input.set_value(invalid.clone(), window, cx));
            let before = serde_json::to_value(&cx.global::<DesktopState>().watch_groups)?;
            page.save_group(cx);
            ensure!(
                page.editor.is_some() && page.error.is_some(),
                "Rejected group edit closed its editor"
            );
            ensure!(
                page.name.read(cx).value().as_str() == invalid,
                "Rejected group edit discarded its input"
            );
            ensure!(
                serde_json::to_value(&cx.global::<DesktopState>().watch_groups)? == before,
                "Rejected group edit changed domain state"
            );
            ensure!(
                !desktop_state::save_now(cx),
                "Unaccepted group text did not block close"
            );
            page.cancel_editor(cx);
            ensure!(
                desktop_state::save_now(cx),
                "Explicit group cancel did not release its close guard"
            );

            let id = desktop_state::dispatch(
                cx,
                Command::SaveWatchGroup {
                    id: None,
                    name: "原子编辑回归".into(),
                    note: String::new(),
                    color: GROUP_COLORS[0],
                    symbols: vec![],
                },
            )?
            .into_id()?;
            let group = cx
                .global::<DesktopState>()
                .watch_groups
                .iter()
                .find(|g| g.id == id)
                .unwrap()
                .clone();
            page.begin_group(Some(group), window, cx);
            page.note.update(cx, |input, cx| {
                input.set_value("用户输入应保留", window, cx)
            });
            desktop_state::dispatch(cx, Command::DeleteWatchGroup(id))?;
            page.save_group(cx);
            ensure!(
                page.editor.is_some()
                    && page.error.as_deref().is_some_and(|e| e.contains("已删除")),
                "Rejected stale group command reported success"
            );
            ensure!(
                page.note.read(cx).value().as_str() == "用户输入应保留",
                "Failed group command lost its draft"
            );
            page.cancel_editor(cx);

            page.begin_rules(window, cx);
            page.rule_inputs[0].update(cx, |input, cx| input.set_value("不是数字", window, cx));
            page.save_rules(cx);
            ensure!(
                page.rules_editor.is_some() && page.error.is_some(),
                "Invalid rule input closed its editor"
            );
            ensure!(
                page.rule_inputs[0].read(cx).value().as_str() == "不是数字",
                "Invalid rule input was replaced"
            );
            ensure!(
                !desktop_state::save_now(cx),
                "Unaccepted rule text did not block close"
            );
            page.rule_inputs[0].update(cx, |input, cx| input.set_value("2.50", window, cx));
            page.save_rules(cx);
            ensure!(
                page.rules_editor.is_none() && page.error.is_none(),
                "Corrected rule edit did not complete"
            );
            ensure!(
                cx.global::<DesktopState>().rules[0].threshold == 2.5,
                "Corrected rule did not reach core"
            );
            ensure!(
                desktop_state::save_now(cx),
                "Successful rule save left a stale close guard"
            );
            desktop_state::dispatch(cx, Command::ReplaceRules(original_rules))?;
            Ok(())
        })
    })??;
    println!(
        "PASS: watch rejected input and stale commands preserve native editors; cancel/retry release pending-input guards"
    );
    Ok(())
}
