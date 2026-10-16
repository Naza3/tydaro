use super::*;

impl WatchPage {
    pub(super) fn group_card(&self, group: &WatchGroup, cx: &mut Context<Self>) -> AnyElement {
        let mut members = group.symbols.clone();
        if self.sorting != 0 {
            members.sort_by(|left, right| {
                let ordering = market::quote(*left, SNAPSHOT_TICK)
                    .change_pct
                    .total_cmp(&market::quote(*right, SNAPSHOT_TICK).change_pct);
                if self.sorting == 1 {
                    ordering.reverse()
                } else {
                    ordering
                }
            });
        }
        let focused = self.selected_group.as_ref() == Some(&group.id);
        let edit_group = group.clone();
        let add_group = group.clone();
        let mut table = div().flex().flex_col().child(
            div()
                .h(px(30.))
                .px_3()
                .flex()
                .items_center()
                .text_size(px(12.))
                .text_color(rgb(MUTED))
                .bg(rgb(0xf8fafb))
                .child(div().flex_1().min_w(px(86.)).child("名称 / 代码"))
                .when(self.actual_columns <= 2, |row| {
                    row.child(div().w(px(66.)).child("走势示意"))
                })
                .child(div().w(px(68.)).text_right().child("最新价"))
                .child(div().w(px(70.)).text_right().child("涨跌幅"))
                .when(self.actual_columns <= 2, |row| {
                    row.child(div().w(px(72.)).text_right().child("月热点*"))
                })
                .child(div().w(px(24.))),
        );
        for symbol in members {
            let quote = market::quote(symbol, SNAPSHOT_TICK);
            let color = if quote.change_pct >= 0. { UP } else { DOWN };
            let item_group = group.clone();
            table = table.child(
                div()
                    .id(SharedString::from(format!(
                        "watch-stock-{}-{}",
                        group.id,
                        symbol.code()
                    )))
                    .test_support()
                    .h(px(49.))
                    .px_3()
                    .flex()
                    .items_center()
                    .border_b_1()
                    .border_color(rgb(0xeff2f5))
                    .cursor_pointer()
                    .bg(rgb(if focused && self.selected_symbol == Some(symbol) {
                        0xeaf6f5
                    } else {
                        0xffffff
                    }))
                    .hover(|style| style.bg(rgb(0xf3f8f8)))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(86.))
                            .flex()
                            .flex_col()
                            .gap(px(2.))
                            .child(div().truncate().child(symbol.name()))
                            .child(small(format!("{} {}", symbol.code(), symbol.market()))),
                    )
                    .when(self.actual_columns <= 2, |row| {
                        row.child(sparkline(symbol, quote.change_pct >= 0., color))
                    })
                    .child(
                        div()
                            .w(px(68.))
                            .text_right()
                            .text_color(rgb(color))
                            .child(format!("{:.2}", quote.price)),
                    )
                    .child(
                        div()
                            .w(px(70.))
                            .text_right()
                            .text_color(rgb(color))
                            .child(format!("{:+.2}%", quote.change_pct)),
                    )
                    .when(self.actual_columns <= 2, |row| {
                        row.child(
                            div()
                                .w(px(72.))
                                .flex()
                                .flex_col()
                                .items_end()
                                .child(div().text_size(px(12.)).child(sector(symbol)))
                                .child(small(format!("{} 次 · 示例", hotspot_count(symbol)))),
                        )
                    })
                    .child(
                        icon_button(
                            SharedString::from(format!(
                                "watch-market-{}-{}",
                                group.id,
                                symbol.code()
                            )),
                            IconName::ChevronRight,
                        )
                        .w(px(24.))
                        .tooltip("打开行情")
                        .on_click(cx.listener(move |_, _, _, cx| {
                            cx.stop_propagation();
                            cx.emit(DesktopEvent::OpenMarket(symbol));
                        })),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.inspect(symbol, Some(item_group.clone()), cx)
                    })),
            );
        }
        if group.symbols.is_empty() {
            let empty_group = group.clone();
            table = table.child(
                div()
                    .h(px(147.))
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap_3()
                    .child(
                        Icon::new(IconName::Folder)
                            .size(px(24.))
                            .text_color(rgb(MUTED)),
                    )
                    .child(small("这个分组还没有股票"))
                    .child(
                        button(
                            SharedString::from(format!("watch-empty-add-{}", group.id)),
                            "添加股票",
                        )
                        .on_click(cx.listener(
                            move |this, _, window, cx| {
                                this.begin_group(Some(empty_group.clone()), window, cx)
                            },
                        )),
                    ),
            );
        }
        let selected = self
            .selected_symbol
            .filter(|symbol| focused && group.symbols.contains(symbol));
        let draft_group = group.clone();
        let footer = div()
            .h(px(36.))
            .px_3()
            .flex()
            .items_center()
            .justify_between()
            .gap_2()
            .child(small(
                selected.map_or("单击速览 · 右箭头打开行情", Symbol::name),
            ))
            .when_some(selected, |row, symbol| {
                row.child(
                    button(
                        SharedString::from(format!("watch-draft-{}", group.id)),
                        "写预案",
                    )
                    .ghost()
                    .text_color(rgb(TEAL))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.emit(DesktopEvent::CreateDraft(
                            this.source(symbol, Some(&draft_group)),
                        ));
                    })),
                )
            })
            .child(
                button(
                    SharedString::from(format!("watch-group-add-{}", group.id)),
                    "添加股票",
                )
                .ghost()
                .icon(IconName::Plus)
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.begin_group(Some(add_group.clone()), window, cx)
                })),
            );
        panel()
            .id(SharedString::from(format!("watch-group-{}", group.id)))
            .test_support()
            .flex_1()
            .min_w_0()
            .overflow_hidden()
            .border_color(rgb(if focused { 0x8ccac7 } else { LINE }))
            .child(
                div()
                    .h(px(42.))
                    .px_3()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .w(px(3.))
                            .h(px(16.))
                            .rounded(px(1.))
                            .bg(rgb(group.color)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(group.name.clone()),
                    )
                    .child(small(group.symbols.len().to_string()))
                    .child(
                        icon_button(
                            SharedString::from(format!("watch-group-edit-{}", group.id)),
                            IconName::Ellipsis,
                        )
                        .tooltip("管理分组")
                        .on_click(cx.listener(
                            move |this, _, window, cx| {
                                this.begin_group(Some(edit_group.clone()), window, cx)
                            },
                        )),
                    ),
            )
            .child(
                div()
                    .px_3()
                    .pb_3()
                    .truncate()
                    .text_size(px(12.))
                    .text_color(rgb(MUTED))
                    .child(if group.note.is_empty() {
                        "添加观察备注，记录这个分组的关注点".to_owned()
                    } else {
                        group.note.clone()
                    }),
            )
            .child(table)
            .child(footer)
            .into_any_element()
    }
}

impl Render for WatchPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = cx.global::<DesktopState>();
        let groups = state.watch_groups.clone();
        let alerts = state.alerts.clone();
        let monitoring = state.monitoring;
        let muted = state.muted;
        let unique = groups
            .iter()
            .flat_map(|group| &group.symbols)
            .collect::<HashSet<_>>()
            .len();
        let width: f32 = window.viewport_size().width.into();
        let available = width - 352.;
        self.actual_columns = if self.columns == 3 && available >= 1_030. {
            3
        } else if self.columns >= 2 && available >= 840. {
            2
        } else {
            1
        };
        let mut layout_buttons = div()
            .flex()
            .items_center()
            .gap_1()
            .p_1()
            .bg(rgb(0xeef2f5))
            .rounded(px(5.));
        for columns in 1..=3 {
            layout_buttons = layout_buttons.child(
                button(
                    SharedString::from(format!("watch-columns-{columns}")),
                    format!("{columns} 列"),
                )
                .ghost()
                .bg(rgb(if self.columns == columns {
                    0xffffff
                } else {
                    0xeef2f5
                }))
                .text_color(rgb(if self.columns == columns { TEAL } else { MUTED }))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.columns = columns;
                    cx.notify();
                })),
            );
        }
        let mut main = div()
            .id("watch-main-scroll")
            .flex_1()
            .min_w_0()
            .min_h_0()
            .h_full()
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .h(px(40.))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(bold("分组看板"))
                    .child(small(format!("{} 组 / {unique} 只股票", groups.len())))
                    .child(div().flex_1())
                    .child(
                        button(
                            "watch-sort",
                            match self.sorting {
                                1 => "涨幅降序",
                                2 => "涨幅升序",
                                _ => "手动顺序",
                            },
                        )
                        .icon(IconName::ArrowUpDown)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.sorting = (this.sorting + 1) % 3;
                            cx.notify();
                        })),
                    )
                    .child(layout_buttons)
                    .child(
                        button("watch-add-group", "新建分组")
                            .icon(IconName::Plus)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.begin_group(None, window, cx)
                            })),
                    ),
            );
        for row in groups.chunks(self.actual_columns) {
            let mut cards = div().flex_shrink_0().flex().items_start().gap_4();
            for group in row {
                cards = cards.child(self.group_card(group, cx));
            }
            for _ in row.len()..self.actual_columns {
                cards = cards.child(div().flex_1());
            }
            main = main.child(cards);
        }
        if groups.is_empty() {
            main = main.child(
                panel()
                    .h(px(260.))
                    .flex_shrink_0()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap_4()
                    .child(
                        Icon::new(IconName::Folder)
                            .size(px(32.))
                            .text_color(rgb(TEAL)),
                    )
                    .child(bold("按自己的观察思路建立分组"))
                    .child(small("例如情绪核心、趋势跟踪、轮动候选。"))
                    .child(
                        button("watch-add-first", "创建第一个分组")
                            .primary()
                            .icon(IconName::Plus)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.begin_group(None, window, cx)
                            })),
                    ),
            );
        }
        main = main
            .child(div().flex_shrink_0().child(self.alerts_panel(&alerts, cx)))
            .child(div().h(px(12.)).flex_shrink_0());
        let mut page = div()
            .id("watch-page")
            .test_support()
            .relative()
            .size_full()
            .min_w_0()
            .min_h_0()
            .bg(rgb(BG))
            .text_color(rgb(INK))
            .text_size(px(14.))
            .p_5()
            .flex()
            .flex_col()
            .gap_5()
            .child(
                div()
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_4()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .text_color(rgb(MUTED))
                                    .child("MARKET MONITOR   盘中观察"),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_3()
                                    .child(
                                        div()
                                            .text_size(px(24.))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child("盯盘"),
                                    )
                                    .child(tag(
                                        if monitoring {
                                            "模拟监控中"
                                        } else {
                                            "监控已暂停"
                                        },
                                        if monitoring { TEAL } else { MUTED },
                                    )),
                            )
                            .child(small("把关注点放在一起，让异动主动来到你面前。")),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(small(format!("{DATE}  14:32 快照")))
                            .child(
                                button(
                                    "watch-muted",
                                    if muted {
                                        "已静音"
                                    } else {
                                        "提示音偏好已开"
                                    },
                                )
                                .icon(if muted {
                                    IconName::VolumeX
                                } else {
                                    IconName::Volume2
                                })
                                .tooltip("仅保存静音偏好，本阶段不播放音频")
                                .on_click(cx.listener(
                                    |this, _, _, cx| this.simple_command(Command::ToggleMuted, cx),
                                )),
                            )
                            .child(
                                button(
                                    "watch-monitor",
                                    if monitoring {
                                        "暂停监控"
                                    } else {
                                        "继续监控"
                                    },
                                )
                                .icon(if monitoring {
                                    IconName::Pause
                                } else {
                                    IconName::Play
                                })
                                .on_click(cx.listener(
                                    |this, _, _, cx| {
                                        this.simple_command(Command::ToggleMonitoring, cx)
                                    },
                                )),
                            ),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .items_start()
                    .gap_4()
                    .child(self.sidebar(&groups, &alerts, cx))
                    .child(main),
            );
        if self.editor.is_some() {
            page = page.child(self.group_editor(cx));
        }
        if self.rules_editor.is_some() {
            page = page.child(self.rules_editor(cx));
        }
        page
    }
}
