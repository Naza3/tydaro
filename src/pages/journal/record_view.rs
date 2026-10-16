use super::*;

impl JournalPage {
    pub(super) fn form_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(form) = &self.form else {
            return div().into_any_element();
        };
        let labels: &[&str] = match form.kind {
            FormKind::Hotspot => &["日期", "热点名称", "高度与观察"],
            FormKind::Buy => &[
                "买入日期",
                "股票名称",
                "板块",
                "方法",
                "手工收益率（%）",
                "备注",
            ],
        };
        let ids = [
            "journal-record-date",
            "journal-record-name",
            "journal-record-sector",
            "journal-record-method",
            "journal-record-profit",
            "journal-record-note",
        ];
        let title = match form.kind {
            FormKind::Hotspot => "人工热点",
            FormKind::Buy => "手工买入记录",
        };
        div()
            .absolute()
            .inset_0()
            .occlude()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgba(0x23334838))
            .child(
                ui::panel()
                    .w(px(580.))
                    .p_5()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(
                        div()
                            .text_size(px(21.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(title),
                    )
                    .child(ui::muted(
                        "内容由你手工维护；不与题材分析、行情或账户成交自动同步。",
                    ))
                    .children(
                        form.fields
                            .iter()
                            .take(form.fields.len().saturating_sub(1))
                            .enumerate()
                            .map(|(index, value)| field(labels[index], ids[index], value)),
                    )
                    .child(multiline(
                        if matches!(form.kind, FormKind::Hotspot) {
                            "高度与观察"
                        } else {
                            "备注"
                        },
                        "journal-record-description",
                        &form.narrative,
                        80.,
                    ))
                    .child(
                        div()
                            .text_color(rgb(UP))
                            .text_size(px(12.))
                            .child(self.error.clone().unwrap_or_default()),
                    )
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .gap_2()
                            .child(
                                action("journal-record-cancel", "取消").on_click(cx.listener(
                                    |this, _, _, cx| {
                                        this.form = None;
                                        this.error = None;
                                        cx.notify();
                                    },
                                )),
                            )
                            .child(
                                action("journal-record-save", "保存记录")
                                    .primary()
                                    .on_click(cx.listener(|this, _, _, cx| this.save_form(cx))),
                            ),
                    ),
            )
            .into_any_element()
    }

    pub(super) fn methods_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(editor) = &self.methods_editor else {
            return div().into_any_element();
        };
        div()
            .id("journal-methods-editor")
            .test_support()
            .absolute()
            .inset_0()
            .occlude()
            .bg(rgba(0x23334850))
            .flex()
            .items_center()
            .justify_center()
            .child(
                ui::panel()
                    .w(px(690.))
                    .p_5()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(
                        div()
                            .text_size(px(21.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("管理买卖方法"),
                    )
                    .child(ui::muted(
                        "每行一个方法，保存时去除空行并去重。修改词库不会重写已保存的预案。",
                    ))
                    .child(
                        div()
                            .flex()
                            .gap_4()
                            .child(div().flex_1().min_w_0().child(multiline(
                                "买入方法",
                                "journal-buy-methods",
                                &editor.buy,
                                230.,
                            )))
                            .child(div().flex_1().min_w_0().child(multiline(
                                "卖出方法",
                                "journal-sell-methods",
                                &editor.sell,
                                230.,
                            ))),
                    )
                    .child(
                        div()
                            .text_color(rgb(UP))
                            .text_size(px(12.))
                            .child(self.error.clone().unwrap_or_default()),
                    )
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .gap_2()
                            .child(
                                action("journal-methods-cancel", "取消").on_click(cx.listener(
                                    |this, _, _, cx| {
                                        this.methods_editor = None;
                                        this.error = None;
                                        cx.notify();
                                    },
                                )),
                            )
                            .child(
                                action("journal-methods-save", "保存词库")
                                    .primary()
                                    .on_click(cx.listener(|this, _, _, cx| this.save_methods(cx))),
                            ),
                    ),
            )
            .into_any_element()
    }

    pub(super) fn buys_history_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut buys = cx.global::<DesktopState>().journal.buys.clone();
        buys.sort_by(|a, b| b.date.cmp(&a.date).then(a.id.cmp(&b.id)));
        div()
            .id("journal-buy-history-view")
            .test_support()
            .absolute()
            .inset_0()
            .occlude()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgba(0x23334838))
            .child(
                ui::panel()
                    .w(px(850.))
                    .h(px(590.))
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .p_4()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .flex_1()
                                    .child(
                                        div()
                                            .text_size(px(21.))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child("全部手工买入记录"),
                                    )
                                    .child(ui::muted("按买入日期降序；手工收益率不代表账户绩效。")),
                            )
                            .child(
                                action("journal-buy-history-close", "关闭")
                                    .ghost()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.buys_history = false;
                                        cx.notify();
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .px_4()
                            .py_2()
                            .flex()
                            .gap_2()
                            .bg(rgb(BG))
                            .text_color(rgb(MUTED))
                            .text_size(px(12.))
                            .child(div().w(px(110.)).child("买入日期"))
                            .child(div().w(px(110.)).child("股票"))
                            .child(div().w(px(110.)).child("板块"))
                            .child(div().flex_1().child("方法"))
                            .child(div().w(px(80.)).child("手工收益"))
                            .child(div().w(px(100.)).child("操作")),
                    )
                    .child(
                        div()
                            .id("journal-buy-history-scroll")
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .children(buys.iter().map(|buy| {
                                let edit_id = buy.id.clone();
                                let delete_id = buy.id.clone();
                                div()
                                    .px_4()
                                    .py_2()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .border_b_1()
                                    .border_color(rgb(LINE))
                                    .text_size(px(13.))
                                    .child(div().w(px(110.)).child(buy.date.clone()))
                                    .child(div().w(px(110.)).child(buy.stock.clone()))
                                    .child(div().w(px(110.)).child(buy.sector.clone()))
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .truncate()
                                            .child(buy.method.clone()),
                                    )
                                    .child(
                                        div()
                                            .w(px(80.))
                                            .text_color(rgb(if buy.profit >= 0. {
                                                UP
                                            } else {
                                                DOWN
                                            }))
                                            .font_weight(if buy.profit > 10. {
                                                FontWeight::BOLD
                                            } else {
                                                FontWeight::NORMAL
                                            })
                                            .child(format!("{:+.2}%", buy.profit)),
                                    )
                                    .child(
                                        div()
                                            .w(px(100.))
                                            .flex()
                                            .gap_1()
                                            .child(
                                                action(
                                                    SharedString::from(format!(
                                                        "journal-history-buy-edit-{}",
                                                        buy.id
                                                    )),
                                                    "编辑",
                                                )
                                                .ghost()
                                                .on_click(cx.listener(
                                                    move |this, _, window, cx| {
                                                        this.start_form(
                                                            FormKind::Buy,
                                                            Some(edit_id.clone()),
                                                            window,
                                                            cx,
                                                        )
                                                    },
                                                )),
                                            )
                                            .child(
                                                action(
                                                    SharedString::from(format!(
                                                        "journal-history-buy-delete-{}",
                                                        buy.id
                                                    )),
                                                    "删除",
                                                )
                                                .ghost()
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    this.delete =
                                                        Some(DeleteTarget::Buy(delete_id.clone()));
                                                    cx.notify();
                                                })),
                                            ),
                                    )
                            })),
                    )
                    .child(
                        div()
                            .p_3()
                            .flex()
                            .items_center()
                            .child(ui::muted(format!("共 {} 条记录", buys.len())))
                            .child(div().flex_1())
                            .child(
                                action("journal-history-add-buy", "＋ 补录买入")
                                    .primary()
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.start_form(FormKind::Buy, None, window, cx)
                                    })),
                            ),
                    ),
            )
            .into_any_element()
    }

    pub(super) fn delete_view(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .absolute()
            .inset_0()
            .occlude()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgba(0x23334838))
            .child(
                ui::panel()
                    .w(px(460.))
                    .p_5()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(
                        div()
                            .text_size(px(20.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("删除这条记录？"),
                    )
                    .child(ui::muted(
                        "删除会修改复盘文档。关闭阅读窗口只关闭视图，不会删除任何记录。",
                    ))
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .gap_2()
                            .child(
                                action("journal-delete-cancel", "保留").on_click(cx.listener(
                                    |this, _, _, cx| {
                                        this.delete = None;
                                        cx.notify();
                                    },
                                )),
                            )
                            .child(
                                action("journal-delete-confirm", "确认删除")
                                    .text_color(rgb(UP))
                                    .on_click(
                                        cx.listener(|this, _, _, cx| this.confirm_delete(cx)),
                                    ),
                            ),
                    ),
            )
            .into_any_element()
    }
}
