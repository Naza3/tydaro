use super::*;

impl LimitPage {
    pub(super) fn matrix(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut matrix = panel().child(
            section_title("多日板块概览")
                .child(tag(format!("最近 {} 个导入日", self.dates().len()), MUTED))
                .child(div().flex_1())
                .child(
                    button(
                        "limit-collapse",
                        if self.collapsed { "展开" } else { "收起" },
                    )
                    .ghost()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.collapsed = !this.collapsed;
                        cx.notify();
                    })),
                ),
        );
        if !self.collapsed {
            matrix = matrix.child(
                div()
                    .px_4()
                    .py_2()
                    .text_size(px(12.))
                    .text_color(rgb(MUTED))
                    .child("每日按原资料顺序排列，列不代表同名板块；数字为资料条数。"),
            );
            for date in self.dates() {
                let day: Vec<_> = self.rows.iter().filter(|r| r.date == date).collect();
                let mut sectors = Vec::new();
                for row in &day {
                    if !sectors.contains(&row.sector) {
                        sectors.push(row.sector.clone());
                    }
                }
                let mut row = div()
                    .h(px(44.))
                    .px_4()
                    .flex()
                    .items_center()
                    .gap_2()
                    .border_t_1()
                    .border_color(rgb(LINE))
                    .child(
                        div()
                            .w(px(76.))
                            .flex_shrink_0()
                            .font_weight(FontWeight::MEDIUM)
                            .child(date[5..].replace('-', ".")),
                    );
                for (index, sector) in sectors.iter().enumerate() {
                    let items: Vec<_> = day.iter().filter(|r| &r.sector == sector).collect();
                    let selected = date == self.selected_date && sector == &self.selected_sector;
                    let matched =
                        self.filters.is_empty() || items.iter().any(|r| matches(r, &self.filters));
                    let next_date = date.clone();
                    let next_sector = sector.clone();
                    row = row.child(
                        div()
                            .id(SharedString::from(format!("limit-sector-{date}-{index}")))
                            .test_support()
                            .flex_1()
                            .min_w_0()
                            .h(px(30.))
                            .px_3()
                            .flex()
                            .items_center()
                            .justify_between()
                            .rounded(px(4.))
                            .border_1()
                            .border_color(rgb(if selected { TEAL } else { LINE }))
                            .bg(rgb(if selected { 0xe4f5f2 } else { 0xf7f9fa }))
                            .text_color(rgb(if matched { sector_color(sector) } else { MUTED }))
                            .cursor_pointer()
                            .child(div().truncate().child(sector.clone()))
                            .child(
                                div()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(items.len().to_string()),
                            )
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.choose(next_date.clone(), next_sector.clone(), cx)
                            })),
                    );
                }
                matrix = matrix.child(
                    row.child(
                        div()
                            .w(px(32.))
                            .text_right()
                            .text_color(rgb(MUTED))
                            .child(day.len().to_string()),
                    ),
                );
            }
            matrix = matrix.child(
                div()
                    .px_4()
                    .py_2()
                    .text_size(px(12.))
                    .text_color(rgb(MUTED))
                    .bg(rgb(0xf8fafb))
                    .child("✓ 原始顺序已保留    ·    示例资料不代表实际涨停结果"),
            );
        }
        matrix.into_any_element()
    }

    pub(super) fn details(&self, cx: &mut Context<Self>) -> AnyElement {
        let visible = self.visible();
        let selected = self.selected();
        let context = source(&selected, PageId::Limit);
        let context_theme = context.clone();
        let context_watch = context.clone();
        let mut details = panel().child(
            section_title("")
                .child(
                    button("limit-details-tab", "板块明细")
                        .when(!self.search_tab, |b| b.primary())
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.search_tab = false;
                            cx.notify();
                        })),
                )
                .child(
                    button("limit-results-tab", "搜索结果")
                        .when(self.search_tab, |b| b.primary())
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.search_tab = true;
                            cx.notify();
                        })),
                )
                .child(tag(format!("{} 条", visible.len()), MUTED))
                .child(div().flex_1())
                .child(
                    button("limit-copy", "复制代码")
                        .disabled(visible.is_empty())
                        .on_click(cx.listener(|this, _, _, cx| {
                            let context = source(&this.visible(), PageId::Limit);
                            cx.write_to_clipboard(ClipboardItem::new_string(
                                context
                                    .symbols
                                    .iter()
                                    .map(|s| s.code())
                                    .collect::<Vec<_>>()
                                    .join("\n"),
                            ));
                            cx.emit(DesktopEvent::Notice(format!(
                                "已复制 {} 个股票代码",
                                context.symbols.len()
                            )));
                        })),
                )
                .child(
                    button("limit-delete-day", "删除整日")
                        .disabled(selected.is_empty() || self.editing.is_some())
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.editing = Some(LimitEdit::Delete(this.selected_date.clone()));
                            cx.notify();
                        })),
                ),
        );
        if !self.search_tab {
            details =
                details.child(
                    div()
                        .p_4()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(
                                    div()
                                        .text_size(px(19.))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child(self.selected_sector.clone()),
                                )
                                .child(tag(self.selected_date.clone(), MUTED))
                                .child(div().flex_1())
                                .child(button("limit-open-theme", "研究题材 →").on_click(
                                    cx.listener(move |_, _, _, cx| {
                                        cx.emit(DesktopEvent::OpenTheme(context_theme.clone()))
                                    }),
                                ))
                                .child(
                                    button("limit-add-watch", "+ 加入盯盘")
                                        .disabled(selected.is_empty())
                                        .on_click(cx.listener(move |_, _, _, cx| {
                                            cx.emit(DesktopEvent::AddToWatch(context_watch.clone()))
                                        })),
                                ),
                        )
                        .child(muted(sector_reason(&self.selected_sector))),
                );
        }
        details = details.child(
            div()
                .h(px(32.))
                .px_4()
                .flex()
                .items_center()
                .bg(rgb(BG))
                .text_size(px(12.))
                .text_color(rgb(MUTED))
                .child(text_cell("股票 / 代码", 142.))
                .child(text_cell("日期 / 连板", 110.))
                .child(div().flex_1().child("涨停原因 / 来源"))
                .child(text_cell("操作", 196.)),
        );
        for row in visible {
            let inspect = source(std::slice::from_ref(&row), PageId::Limit);
            let draft = inspect.clone();
            let edit_row = row.clone();
            let locate = row.clone();
            details = details.child(
                div()
                    .id(SharedString::from(format!("limit-row-{}", row.id)))
                    .min_h(px(66.))
                    .px_4()
                    .py_2()
                    .flex()
                    .items_center()
                    .border_t_1()
                    .border_color(rgb(LINE))
                    .bg(rgb(
                        if self.selected_code == row.symbol && !self.search_tab {
                            0xf1f9f8
                        } else {
                            0xffffff
                        },
                    ))
                    .child(
                        div()
                            .w(px(142.))
                            .flex_shrink_0()
                            .child(
                                button(
                                    SharedString::from(format!("limit-inspect-{}", row.id)),
                                    row.name.clone(),
                                )
                                .ghost()
                                .on_click(cx.listener(
                                    move |_, _, _, cx| {
                                        cx.emit(DesktopEvent::Inspect(inspect.clone()))
                                    },
                                )),
                            )
                            .child(muted(row.symbol.code())),
                    )
                    .child(
                        div()
                            .w(px(110.))
                            .flex_shrink_0()
                            .text_size(px(12.))
                            .child(row.date.clone())
                            .child(tag(row.board.clone(), TEAL)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .pr_3()
                            .text_size(px(13.))
                            .child(row.reason.clone())
                            .child(muted(row.source.clone())),
                    )
                    .child(
                        div()
                            .w(px(196.))
                            .flex_shrink_0()
                            .flex()
                            .gap_1()
                            .child(
                                button(
                                    SharedString::from(format!("limit-edit-{}", row.id)),
                                    "编辑",
                                )
                                .ghost()
                                .on_click(cx.listener(
                                    move |this, _, window, cx| {
                                        this.begin_edit(edit_row.clone(), window, cx)
                                    },
                                )),
                            )
                            .when(self.search_tab, |view| {
                                view.child(
                                    button(
                                        SharedString::from(format!("limit-locate-{}", row.id)),
                                        "定位",
                                    )
                                    .ghost()
                                    .on_click(cx.listener(
                                        move |this, _, _, cx| {
                                            this.choose(
                                                locate.date.clone(),
                                                locate.sector.clone(),
                                                cx,
                                            );
                                            this.selected_code = locate.symbol;
                                        },
                                    )),
                                )
                            })
                            .child(
                                button(
                                    SharedString::from(format!("limit-draft-{}", row.id)),
                                    "引用预案",
                                )
                                .ghost()
                                .on_click(cx.listener(
                                    move |_, _, _, cx| {
                                        cx.emit(DesktopEvent::CreateDraft(draft.clone()))
                                    },
                                )),
                            ),
                    ),
            );
        }
        if self.visible().is_empty() {
            details = details.child(
                div()
                    .p_8()
                    .text_color(rgb(MUTED))
                    .child("没有匹配资料。移除条件或选择其他日期与板块。"),
            );
        }
        details.into_any_element()
    }
}

impl Render for LimitPage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let owner = cx.weak_entity();
        let result_count = self
            .rows
            .iter()
            .filter(|row| matches(row, &self.filters))
            .count();
        page()
            .id("limit-page")
            .test_support()
            .overflow_y_scroll()
            .gap_3()
            .child(
                div()
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(heading(
                        "整理资料 · 发现线索",
                        "涨停资料",
                        "按导入日追溯板块与个股，保留每一条来源",
                    ))
                    .child(
                        button("limit-import", "导入示例资料")
                            .disabled(self.load_error.is_some() || self.editing.is_some())
                            .primary()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.editing = Some(LimitEdit::Import);
                                cx.notify();
                            })),
                    ),
            )
            .when_some(self.load_error.clone(), |view, message| {
                view.child(
                    div()
                        .flex_shrink_0()
                        .text_color(rgb(0xc92f57))
                        .child(message),
                )
            })
            .child(
                panel()
                    .p_3()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().text_color(rgb(MUTED)).child("筛选资料"))
                    .child(
                        button("limit-filter-field", self.field.label()).dropdown_menu(
                            move |mut menu, _, _| {
                                for field in Field::ALL {
                                    let owner = owner.clone();
                                    menu = menu.item(PopupMenuItem::new(field.label()).on_click(
                                        move |_, _, cx| {
                                            let _ = owner.update(cx, |this, cx| {
                                                this.field = field;
                                                cx.notify();
                                            });
                                        },
                                    ));
                                }
                                menu
                            },
                        ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(Input::new(&self.query).id("limit-filter")),
                    )
                    .child(
                        button("limit-filter-add", "+ 添加条件")
                            .disabled(self.query.read(cx).value().trim().is_empty())
                            .on_click(cx.listener(|this, _, _, cx| this.add_filter(cx))),
                    )
                    .child(muted("AND · 同时满足")),
            )
            .when(!self.filters.is_empty(), |view| {
                view.child(
                    div()
                        .flex_shrink_0()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap_2()
                        .children(self.filters.iter().enumerate().map(|(index, filter)| {
                            button(
                                SharedString::from(format!("limit-filter-remove-{index}")),
                                format!("{}：{} ×", filter.field.label(), filter.value),
                            )
                            .on_click(cx.listener(
                                move |this, _, _, cx| {
                                    this.filters.remove(index);
                                    cx.notify();
                                },
                            ))
                        }))
                        .child(button("limit-filter-clear", "清空条件").ghost().on_click(
                            cx.listener(|this, _, _, cx| {
                                this.filters.clear();
                                this.search_tab = false;
                                cx.notify();
                            }),
                        ))
                        .child(
                            muted(format!("{result_count} 条命中"))
                                .id("limit-result-count")
                                .aria_label(format!("{result_count} 条命中"))
                                .test_support(),
                        ),
                )
            })
            .when(self.editing.is_some(), |view| view.child(self.editor(cx)))
            .when(!self.deleted.is_empty(), |view| {
                view.child(
                    div()
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .gap_3()
                        .child(muted(format!("已删除 {} 条资料", self.deleted.len())))
                        .child(
                            button("limit-undo-delete", "撤销整日删除")
                                .disabled(self.editing.is_some())
                                .on_click(cx.listener(|this, _, _, cx| this.undo_delete(cx))),
                        ),
                )
            })
            .child(self.matrix(cx))
            .child(self.details(cx))
            .child(
                muted(
                    "资料修改保存在本地工作台文档中。题材页是独立研究样例，不随这些资料自动重算。",
                )
                .flex_shrink_0(),
            )
    }
}
