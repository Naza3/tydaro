use super::*;

impl JournalPage {
    pub(super) fn editor_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(e) = &self.editor else {
            return div().into_any_element();
        };
        let selected = e.draft.plan.symbols.clone();
        div().id("journal-editor").test_support().absolute().inset_0().occlude().bg(rgba(0x23334838)).flex().justify_end()
            .child(div().w(px(850.)).h_full().min_h_0().flex().flex_col().bg(rgb(0xffffff)).border_l_1().border_color(rgb(LINE))
                .child(div().p_5().flex_shrink_0().flex().items_center().gap_3().border_b_1().border_color(rgb(LINE))
                    .child(div().flex_1().child(div().text_size(px(21.)).font_weight(FontWeight::SEMIBOLD).child(if e.draft.editing_id.is_some() { "编辑预案" } else { "新的研究预案" }))
                        .child(ui::muted(format!("{} · 草稿自动保留，完整预案需要明确触发条件", e.draft.plan.date))))
                    .child(action("journal-editor-close", "关闭").ghost().on_click(cx.listener(|this, _, window, cx| { this.close_editor(false, window, cx); }))))
                .child(div().id("journal-editor-scroll").flex_1().min_h_0().overflow_y_scroll().track_scroll(&e.scroll).p_5().flex().flex_col().gap_4()
                    .child(field("板块 / 主题 *", "journal-sector", &e.sector))
                    .child(multiline("研究依据", "journal-basis", &e.basis, 72.))
                    .child(div().flex().flex_col().flex_shrink_0().gap_2().child(ui::muted("关联股票 · 可多选"))
                        .child(div().flex().flex_wrap().gap_1().children(Symbol::ALL.iter().filter(|symbol| symbol.currency() == "CNY").map(|symbol| {
                            let symbol = *symbol;
                            action(SharedString::from(format!("journal-pick-{}", symbol.code())), symbol.name()).ghost()
                                .bg(rgb(if selected.contains(&symbol) { 0xe2f4f2 } else { 0xf5f7f9 })).text_color(rgb(if selected.contains(&symbol) { TEAL } else { MUTED }))
                                .on_click(cx.listener(move |this, _, _, cx| this.toggle_symbol(symbol, cx)))
                        }))))
                    .child(div().flex().flex_shrink_0().items_center().child(ui::muted("方法可以自由输入，也可以选取你维护的常用方法。"))
                        .child(div().flex_1()).child(action("journal-methods-open", "管理方法词库").ghost().text_color(rgb(TEAL))
                            .on_click(cx.listener(|this, _, window, cx| this.open_methods(window, cx)))))
                    .child(div().flex().flex_shrink_0().gap_4()
                        .child(ui::panel().flex_1().p_4().flex().flex_col().gap_3().child(ui::tag("买入计划", TEAL))
                            .child(field("买入方法", "journal-method", &e.method))
                            .child(self.method_chips(false, cx))
                            .child(multiline("买入触发条件", "journal-entry", &e.entry, 85.))
                            .child(multiline("放弃买入条件", "journal-avoid", &e.avoid, 72.)))
                        .child(ui::panel().flex_1().p_4().flex().flex_col().gap_3().child(ui::tag("卖出计划", UP))
                            .child(field("卖出方法", "journal-sell-method", &e.sell_method))
                            .child(self.method_chips(true, cx))
                            .child(multiline("卖出触发条件", "journal-exit", &e.exit, 85.))
                            .child(multiline("继续持有条件", "journal-hold", &e.hold, 72.))))
                    .child(ui::muted(format!("来源：{}", e.draft.plan.source)).flex_shrink_0())
                    .child(ui::muted("允许只写买入或只写卖出。已填写的一侧须同时有方法和触发条件；未完成内容请保留草稿。").flex_shrink_0()))
                .child(div().p_4().flex_shrink_0().border_t_1().border_color(rgb(LINE)).flex().items_center().gap_2()
                    .child(div().flex_1().text_size(px(12.)).text_color(rgb(UP)).child(self.error.clone().unwrap_or_default()))
                    .child(action("journal-draft-save", "保留草稿").on_click(cx.listener(|this, _, window, cx| { this.close_editor(true, window, cx); })))
                    .child(action("journal-plan-save", "保存为预案").primary().on_click(cx.listener(|this, _, window, cx| { this.save_plan(cx); if this.editor.is_none() { window.focus(&this.focus, cx); } }))))).into_any_element()
    }

    pub(super) fn method_chips(&self, sell: bool, cx: &mut Context<Self>) -> Div {
        let journal = &cx.global::<DesktopState>().journal;
        let methods = if sell {
            journal.sell_methods.clone()
        } else {
            journal.methods.clone()
        };
        div()
            .flex()
            .flex_wrap()
            .gap_1()
            .children(methods.iter().take(6).enumerate().map(|(index, method)| {
                let value = method.clone();
                action(
                    SharedString::from(format!("journal-method-chip-{sell}-{index}")),
                    method.clone(),
                )
                .ghost()
                .text_color(rgb(TEAL))
                .on_click(cx.listener(move |this, _, window, cx| {
                    if let Some(editor) = &this.editor {
                        let entity = if sell {
                            editor.sell_method.clone()
                        } else {
                            editor.method.clone()
                        };
                        entity.update(cx, |input, cx| input.set_value(value.clone(), window, cx));
                        this.persist_editor(cx);
                    }
                    cx.notify();
                }))
            }))
    }
}
