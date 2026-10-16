use super::*;

impl JournalPage {
    pub(super) fn plan_card(&self, plan: &Plan, cx: &mut Context<Self>) -> AnyElement {
        let editing = plan.clone();
        let watch = plan_context(plan);
        let delete_date = plan.date.clone();
        let delete_id = plan.id.clone();
        div()
            .id(SharedString::from(format!("journal-plan-{}", plan.id)))
            .test_support()
            .p_4()
            .flex()
            .flex_col()
            .flex_shrink_0()
            .gap_3()
            .bg(rgb(0xffffff))
            .border_1()
            .border_color(rgb(LINE))
            .rounded_lg()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(17.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(plan.sector.clone()),
                    )
                    .child(ui::tag(
                        if plan.method.is_empty() {
                            "卖出计划"
                        } else {
                            "买卖预案"
                        },
                        TEAL,
                    ))
                    .child(
                        action(
                            SharedString::from(format!("journal-edit-{}", plan.id)),
                            "编辑",
                        )
                        .ghost()
                        .on_click(cx.listener(
                            move |this, _, window, cx| this.edit_plan(editing.clone(), window, cx),
                        )),
                    )
                    .child(
                        action(
                            SharedString::from(format!("journal-delete-{}", plan.id)),
                            "删除",
                        )
                        .ghost()
                        .text_color(rgb(MUTED))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.delete =
                                Some(DeleteTarget::Plan(delete_date.clone(), delete_id.clone()));
                            cx.notify();
                        })),
                    ),
            )
            .child(div().text_size(px(13.)).text_color(rgb(MUTED)).child(
                if plan.basis.is_empty() {
                    "尚未填写研究依据".to_string()
                } else {
                    plan.basis.clone()
                },
            ))
            .child(
                div()
                    .flex()
                    .gap_5()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(ui::tag(
                                format!(
                                    "买入 · {}",
                                    if plan.method.is_empty() {
                                        "未计划"
                                    } else {
                                        &plan.method
                                    }
                                ),
                                TEAL,
                            ))
                            .child(text_block("触发条件", &plan.entry, TEAL))
                            .child(text_block("放弃条件", &plan.avoid, MUTED)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(ui::tag(
                                format!(
                                    "卖出 · {}",
                                    if plan.sell_method.is_empty() {
                                        "未计划"
                                    } else {
                                        &plan.sell_method
                                    }
                                ),
                                UP,
                            ))
                            .child(text_block("离场条件", &plan.exit, UP))
                            .child(text_block("继续持有", &plan.hold, MUTED)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_2()
                    .pt_2()
                    .border_t_1()
                    .border_color(rgb(LINE))
                    .children(plan.symbols.iter().map(|symbol| {
                        let symbol = *symbol;
                        let mut context = plan_context(plan);
                        context.symbols = vec![symbol];
                        action(
                            SharedString::from(format!(
                                "journal-symbol-{}-{}",
                                plan.id,
                                symbol.code()
                            )),
                            format!("{} {}", symbol.name(), symbol.code()),
                        )
                        .ghost()
                        .text_color(rgb(TEAL))
                        .on_click(cx.listener(move |_, _, _, cx| {
                            cx.emit(DesktopEvent::Inspect(context.clone()))
                        }))
                    }))
                    .child(div().flex_1())
                    .child(ui::muted(plan.source.clone()))
                    .child(
                        action(
                            SharedString::from(format!("journal-watch-{}", plan.id)),
                            "加入盯盘 ↗",
                        )
                        .ghost()
                        .text_color(rgb(TEAL))
                        .on_click(cx.listener(move |_, _, _, cx| {
                            cx.emit(DesktopEvent::AddToWatch(watch.clone()))
                        })),
                    ),
            )
            .into_any_element()
    }
}

impl Render for JournalPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let journal = cx.global::<DesktopState>().journal.clone();
        let day = journal.days.get(&self.date).cloned().unwrap_or_default();
        sync_shared_note(&self.note, &mut self.note_synced, &day.note, window, cx);
        let mut hotspots = journal.hotspots.clone();
        hotspots.sort_by(|a, b| b.date.cmp(&a.date).then(b.created.cmp(&a.created)));
        let buys = journal.recent_buys();
        let plans = day
            .plans
            .iter()
            .map(|plan| self.plan_card(plan, cx))
            .collect::<Vec<_>>();
        ui::page().id("journal-page").test_support().relative().track_focus(&self.focus)
            .child(div().flex().items_center().gap_4().flex_shrink_0()
                .child(div().flex_1().child(ui::heading("REVIEW / 从观察走向决策", "复盘", "先写条件，再看结果。把研究整理成可以回顾的预案。")))
                .child(div().w(px(145.)).child(Input::new(&self.date_input).id("journal-date-input").w_full()))
                .child(action("journal-date-apply", "切换日期").on_click(cx.listener(|this, _, window, cx| {
                    let date = this.date_input.read(cx).value().to_string();
                    if desktop_state::valid_date(&date) { this.select_date(&date, window, cx); }
                    else { this.error = Some("请输入有效日期 YYYY-MM-DD".into()); cx.notify(); }
                })))
                .child(action("journal-reading-open", "阅读窗口 ↗").on_click(cx.listener(|this, _, window, cx| this.open_reading(window, cx))))
                .child(action("journal-new-plan", "＋ 写预案").primary().on_click(cx.listener(|this, _, window, cx| this.create(window, cx)))))
            .child(div().flex_1().min_h_0().flex().gap_4()
                .child(div().flex_1().min_w_0().min_h_0().flex().flex_col().gap_3()
                    .child(ui::panel().p_3().flex_shrink_0().flex().flex_col().gap_2()
                        .child(div().flex().items_center().gap_2().child(div().flex_1().font_weight(FontWeight::SEMIBOLD).child("人工热点"))
                            .child(ui::muted(format!("{}/5 · 仅手工维护", hotspots.len())))
                            .child(action("journal-add-hotspot", "＋ 添加").ghost().text_color(rgb(TEAL)).on_click(cx.listener(|this, _, window, cx| this.start_form(FormKind::Hotspot, None, window, cx)))))
                        .child(div().flex().gap_2().children(hotspots.iter().map(|hot| {
                            let edit_id = hot.id.clone(); let delete_id = hot.id.clone();
                            div().flex_1().min_w_0().p_2().bg(rgb(0xf6f9fa)).rounded_md().flex().flex_col().gap_1()
                                .child(div().flex().items_center().child(div().flex_1().truncate().font_weight(FontWeight::MEDIUM).child(hot.sector.clone()))
                                    .child(action(SharedString::from(format!("journal-hot-delete-{}", hot.id)), "×").ghost().on_click(cx.listener(move |this, _, _, cx| { this.delete = Some(DeleteTarget::Hotspot(delete_id.clone())); cx.notify(); }))))
                                .child(ui::muted(hot.heights.clone()))
                                .child(action(SharedString::from(format!("journal-hot-edit-{}", hot.id)), format!("{} · 编辑", hot.date)).ghost().text_color(rgb(MUTED)).on_click(cx.listener(move |this, _, window, cx| this.start_form(FormKind::Hotspot, Some(edit_id.clone()), window, cx))))
                        }))))
                    .child(div().flex().items_center().gap_2().child(div().text_size(px(17.)).font_weight(FontWeight::SEMIBOLD).child(format!("{} 的预案", self.date)))
                        .child(ui::tag(format!("{} 条预案", day.plans.len()), TEAL)).child(ui::muted(format!("{} 份草稿", day.drafts.len()))))
                    .child(div().id("journal-document-scroll").test_support().flex_1().min_h_0().overflow_y_scroll().flex().flex_col().gap_3()
                        .when(day.plans.is_empty() && day.drafts.is_empty(), |el| el.child(ui::panel().p_5().child(ui::muted("这一天还没有预案。可以手工新建，也可以从题材或涨停研究中引用草稿。"))))
                        .children(day.drafts.iter().map(|draft| {
                            let open_id = draft.plan.id.clone(); let remove_id = draft.plan.id.clone(); let date = draft.plan.date.clone();
                            div().p_3().flex_shrink_0().border_1().border_color(rgb(0xcfe4e2)).bg(rgb(0xf2fbfa)).rounded_md().flex().items_center().gap_2()
                                .child(ui::tag("草稿", TEAL)).child(div().flex_1().child(if draft.plan.sector.is_empty() { "未命名研究预案".into() } else { draft.plan.sector.clone() }))
                                .child(action(SharedString::from(format!("journal-draft-open-{}", draft.plan.id)), "继续编辑").ghost().text_color(rgb(TEAL)).on_click(cx.listener(move |this, _, window, cx| this.open_draft(&open_id, window, cx))))
                                .child(action(SharedString::from(format!("journal-draft-delete-{}", draft.plan.id)), "删除").ghost().on_click(cx.listener(move |this, _, _, cx| { this.delete = Some(DeleteTarget::Draft(date.clone(), remove_id.clone())); cx.notify(); })))
                        }))
                        .children(plans)
                        .child(ui::panel().p_4().flex_shrink_0().flex().flex_col().gap_2().child(div().font_weight(FontWeight::SEMIBOLD).child("盘中观察与复盘笔记"))
                            .child(ui::muted("自动保存到当前日期。阅读窗口同步内容，窗口布局彼此独立。"))
                            .child(multiline("记录", "journal-note", &self.note, 120.)))))
                .child(div().w(px(260.)).flex_shrink_0().min_h_0().flex().flex_col().gap_3()
                    .child(ui::panel().p_3().flex().flex_col().gap_2()
                        .child(div().font_weight(FontWeight::SEMIBOLD).child("历史复盘"))
                        .children(journal.days.iter().rev().take(6).map(|(date, day)| {
                            let date = date.clone(); let current = date == self.date;
                            action(SharedString::from(format!("journal-history-{date}")), format!("{date}    {} 预案 · {} 草稿", day.plans.len(), day.drafts.len())).ghost().w_full()
                                .bg(rgb(if current { 0xe5f5f3 } else { 0xffffff })).text_color(rgb(if current { TEAL } else { INK }))
                                .on_click(cx.listener(move |this, _, window, cx| this.select_date(&date, window, cx)))
                        })))
                    .child(ui::panel().flex_1().min_h_0().flex().flex_col()
                        .child(div().p_3().flex().flex_col().gap_1().child(div().flex().items_center().child(div().flex_1().font_weight(FontWeight::SEMIBOLD).child("最近买入记录"))
                            .child(action("journal-add-buy", "＋").ghost().on_click(cx.listener(|this, _, window, cx| this.start_form(FormKind::Buy, None, window, cx)))))
                            .child(div().flex().items_center().child(ui::muted("最近 10 笔 · 按收益率展示"))
                                .child(div().flex_1()).child(action("journal-buy-history", "全部记录").ghost().text_color(rgb(TEAL)).on_click(cx.listener(|this, _, _, cx| { this.buys_history = true; cx.notify(); })))))
                        .child(div().id("journal-buys-scroll").flex_1().min_h_0().overflow_y_scroll().children(buys.iter().map(|buy| {
                            let edit_id = buy.id.clone(); let delete_id = buy.id.clone();
                            div().px_3().py_2().border_t_1().border_color(rgb(LINE)).flex().flex_col().gap_1()
                                .child(div().flex().items_center().gap_1().child(div().flex_1().font_weight(FontWeight::MEDIUM).child(buy.stock.clone()))
                                    .child(div().text_color(rgb(if buy.profit >= 0. { UP } else { DOWN })).font_weight(if buy.profit > 10. { FontWeight::BOLD } else { FontWeight::NORMAL }).child(format!("{:+.1}%", buy.profit))))
                                .child(ui::muted(format!("{} · {}", buy.date, buy.method)))
                                .child(div().flex().gap_1().child(action(SharedString::from(format!("journal-buy-edit-{}", buy.id)), "编辑").ghost().on_click(cx.listener(move |this, _, window, cx| this.start_form(FormKind::Buy, Some(edit_id.clone()), window, cx))))
                                    .child(action(SharedString::from(format!("journal-buy-delete-{}", buy.id)), "删除").ghost().text_color(rgb(MUTED)).on_click(cx.listener(move |this, _, _, cx| { this.delete = Some(DeleteTarget::Buy(delete_id.clone())); cx.notify(); }))))
                        })))
                        .child(div().p_3().border_t_1().border_color(rgb(LINE)).child(ui::muted("收益率为手工输入，不代表账户绩效。仅 >10% 加粗。"))))))
            .when_some(self.note_error.clone(), |el, error| el.child(div().id("journal-note-error").test_support().text_size(px(12.)).text_color(rgb(UP)).child(error)))
            .when(self.message.is_some() || (self.error.is_some() && !self.is_editing()), |el| el.child(div().text_size(px(12.)).text_color(rgb(if self.error.is_some() { UP } else { MUTED })).child(self.error.clone().or(self.message.clone()).unwrap_or_default())))
            .when(self.editor.is_some(), |el| el.child(self.editor_view(cx)))
            .when(self.buys_history, |el| el.child(self.buys_history_view(cx)))
            .when(self.methods_editor.is_some(), |el| el.child(self.methods_view(cx)))
            .when(self.form.is_some(), |el| el.child(self.form_view(cx)))
            .when(self.delete.is_some(), |el| el.child(self.delete_view(cx)))
    }
}
