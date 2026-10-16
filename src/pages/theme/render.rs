use super::*;

impl ThemePage {
    pub(super) fn evidence(&self, object: &ThemeObject, cx: &mut Context<Self>) -> AnyElement {
        let mut dates: Vec<_> = EVIDENCE_DATES
            .into_iter()
            .filter(|d| *d <= object.trigger)
            .take(4)
            .collect();
        dates.reverse();
        let symbol = object.symbol;
        let watch = self.context(object, vec![symbol], cx);
        let draft = watch.clone();
        let mut body = div().flex_1().min_w_0().flex().flex_col().gap_3().child(
            panel()
                .p_4()
                .flex()
                .flex_col()
                .gap_3()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_3()
                        .child(
                            div()
                                .size(px(36.))
                                .rounded(px(8.))
                                .bg(rgb(0xe2f4f0))
                                .text_color(rgb(TEAL))
                                .flex()
                                .items_center()
                                .justify_center()
                                .font_weight(FontWeight::BOLD)
                                .child(
                                    object
                                        .symbol
                                        .name()
                                        .chars()
                                        .next()
                                        .unwrap_or('研')
                                        .to_string(),
                                ),
                        )
                        .child(
                            div()
                                .flex_1()
                                .child(
                                    div()
                                        .text_size(px(22.))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child(object.symbol.name()),
                                )
                                .child(muted(object.symbol.code())),
                        )
                        .child(
                            button("theme-open-market", "查看行情 ↗").on_click(cx.listener(
                                move |_, _, _, cx| cx.emit(DesktopEvent::OpenMarket(symbol)),
                            )),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap_2()
                        .child(tag(object.sector, TEAL))
                        .child(tag(object.method, MUTED))
                        .child(div().flex_1())
                        .child(
                            button("theme-add-watch", "+ 加入盯盘").on_click(cx.listener(
                                move |_, _, _, cx| cx.emit(DesktopEvent::AddToWatch(watch.clone())),
                            )),
                        )
                        .child(
                            button("theme-create-draft", "引用为预案 →")
                                .primary()
                                .on_click(cx.listener(move |_, _, _, cx| {
                                    cx.emit(DesktopEvent::CreateDraft(draft.clone()))
                                })),
                        ),
                )
                .child(line())
                .child(div().flex().items_center().justify_between().children(
                    dates.iter().enumerate().map(|(index, date)| {
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap_1()
                            .child(tag(format!("{} 板", index + 1), TEAL))
                            .child(muted(date[5..].replace('-', ".")))
                    }),
                ))
                .child(muted("历史归属固定于四板周期；四日指已导入日。")),
        );
        let mut evidence = panel().child(
            section_title("")
                .child(
                    button("theme-cycle-tab", "周期证据")
                        .when(!self.keyword_tab, |b| b.primary())
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.keyword_tab = false;
                            cx.notify();
                        })),
                )
                .child(
                    button("theme-keywords-tab", "八日关键词")
                        .when(self.keyword_tab, |b| b.primary())
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.keyword_tab = true;
                            cx.notify();
                        })),
                ),
        );
        if self.keyword_tab {
            evidence = evidence.child(
                div()
                    .p_4()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(format!("关键词：{}", object.keyword)),
                    )
                    .child(muted(
                        "向前八个已导入日的命中样例，模糊命中不是确定业务关联。",
                    )),
            );
            let peers: Vec<_> = object
                .related
                .iter()
                .chain(&object.generic)
                .copied()
                .collect();
            for (index, date) in EVIDENCE_DATES
                .into_iter()
                .filter(|d| *d <= object.trigger)
                .take(8)
                .enumerate()
            {
                let peer = peers[index % peers.len()];
                let mut context = self.context(object, vec![peer], cx);
                context.date = date.into();
                evidence = evidence.child(
                    div()
                        .min_h(px(43.))
                        .px_4()
                        .flex()
                        .items_center()
                        .gap_2()
                        .border_t_1()
                        .border_color(rgb(LINE))
                        .child(muted(date[5..].replace('-', ".")))
                        .child(
                            button(
                                SharedString::from(format!("theme-keyword-{index}")),
                                peer.name(),
                            )
                            .ghost()
                            .on_click(cx.listener(
                                move |_, _, _, cx| cx.emit(DesktopEvent::Inspect(context.clone())),
                            )),
                        )
                        .child(div().flex_1().text_size(px(12.)).child(if index % 3 == 0 {
                            object.keyword.to_owned()
                        } else {
                            format!("{}…", object.keyword.chars().take(2).collect::<String>())
                        }))
                        .child(tag(
                            if index % 3 == 0 { "完全" } else { "模糊" },
                            if index % 3 == 0 { TEAL } else { 0xb58a50 },
                        )),
                );
            }
        } else {
            let metrics = [
                ("涨停累计", object.counts.iter().sum::<u32>()),
                ("首板累计", object.first.iter().sum::<u32>()),
                ("二板累计", object.second.iter().sum::<u32>()),
            ];
            evidence = evidence
                .child(
                    div()
                        .p_4()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(div().font_weight(FontWeight::SEMIBOLD).child("板块梯队"))
                        .child(muted("四个已导入日累计次数，同股跨日分别计数"))
                        .child(div().flex().gap_3().children(metrics.into_iter().map(
                            |(label, count)| {
                                div()
                                    .flex_1()
                                    .p_3()
                                    .rounded(px(6.))
                                    .bg(rgb(BG))
                                    .child(muted(label))
                                    .child(
                                        div()
                                            .text_size(px(25.))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child(format!("{count} 次")),
                                    )
                            },
                        )))
                        .child(
                            div()
                                .h(px(146.))
                                .flex()
                                .items_end()
                                .gap_4()
                                .px_4()
                                .children(dates.iter().enumerate().map(|(index, date)| {
                                    div()
                                        .flex_1()
                                        .flex()
                                        .flex_col()
                                        .items_center()
                                        .gap_1()
                                        .child(
                                            div()
                                                .text_size(px(12.))
                                                .text_color(rgb(TEAL))
                                                .child(object.counts[index].to_string()),
                                        )
                                        .child(
                                            div()
                                                .w(px(42.))
                                                .h(px(object.counts[index] as f32 * 9.))
                                                .rounded_t(px(4.))
                                                .bg(rgb(0x69b9b0))
                                                .flex()
                                                .items_end()
                                                .child(
                                                    div()
                                                        .w_full()
                                                        .h(px(object.second[index] as f32 * 9.))
                                                        .bg(rgb(TEAL)),
                                                ),
                                        )
                                        .child(muted(date[5..].replace('-', ".")))
                                })),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .child(
                                    div()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child("周期内题材归属"),
                                )
                                .child(
                                    button(
                                        "theme-specific-only",
                                        if self.specific_only {
                                            "✓ 仅具体题材"
                                        } else {
                                            "仅具体题材"
                                        },
                                    )
                                    .ghost()
                                    .on_click(cx.listener(
                                        |this, _, _, cx| {
                                            this.specific_only = !this.specific_only;
                                            cx.notify();
                                        },
                                    )),
                                ),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .px_4()
                        .py_2()
                        .bg(rgb(BG))
                        .text_size(px(12.))
                        .text_color(rgb(MUTED))
                        .child(div().flex_1().child("归属题材"))
                        .child(text_cell("出现次数", 90.))
                        .child(text_cell("日期覆盖", 90.)),
                )
                .children(
                    [
                        (object.sector, "4 次", "全部 4 日"),
                        (object.secondary, "3 次", "第 2—4 日"),
                        ("其他", "1 次", "第 1 日"),
                    ]
                    .into_iter()
                    .filter(|(name, _, _)| !self.specific_only || *name != "其他")
                    .map(|(name, count, coverage)| {
                        div()
                            .px_4()
                            .h(px(38.))
                            .flex()
                            .items_center()
                            .border_t_1()
                            .border_color(rgb(LINE))
                            .child(div().flex_1().child(name))
                            .child(text_cell(count, 90.))
                            .child(text_cell(coverage, 90.))
                    }),
                );
        }
        body = body.child(evidence).child(muted(
            "引用研究依据会创建独立草稿，已有预案和人工热点保持原样。",
        ));
        body.into_any_element()
    }

    pub(super) fn related(&self, object: &ThemeObject, cx: &mut Context<Self>) -> AnyElement {
        let requested = self.requested.read(cx).value();
        let actual = actual_date(&requested);
        let mut view = panel()
            .flex()
            .flex_col()
            .child(section_title("截止日联动").child(tag(actual.unwrap_or("无数据"), MUTED)))
            .child(
                div()
                    .p_4()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(muted("历史归属 × 截止日资料"))
                    .child(
                        div()
                            .text_size(px(13.))
                            .child("同题材与关键词提供两条观察线索。"),
                    ),
            );
        for (label, generic, peers) in [
            ("同板块", false, &object.related),
            ("泛板块关键词", true, &object.generic),
        ] {
            view = view.child(
                div()
                    .px_4()
                    .py_2()
                    .bg(rgb(BG))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(format!("{label}   {} 只", peers.len())),
            );
            view = view.child(div().px_4().py_2().child(muted(if generic {
                "仅匹配“公告 / 其他”等泛板块资料"
            } else {
                "四板周期具体题材在截止日的其他股票"
            })));
            for (index, peer) in peers.iter().enumerate() {
                let inspect = self.context(object, vec![*peer], cx);
                let watch = inspect.clone();
                view = view.child(
                    div()
                        .p_4()
                        .border_t_1()
                        .border_color(rgb(LINE))
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(
                                    button(
                                        SharedString::from(format!("theme-peer-{}", peer.code())),
                                        peer.name(),
                                    )
                                    .ghost()
                                    .on_click(cx.listener(
                                        move |_, _, _, cx| {
                                            cx.emit(DesktopEvent::Inspect(inspect.clone()))
                                        },
                                    )),
                                )
                                .child(div().flex_1())
                                .child(tag(
                                    if generic {
                                        "模糊"
                                    } else if index == 0 {
                                        "2天2板"
                                    } else {
                                        "首板"
                                    },
                                    if generic { 0xb58a50 } else { TEAL },
                                )),
                        )
                        .child(muted(format!(
                            "{}  ·  {}",
                            peer.code(),
                            if generic {
                                object.keyword
                            } else {
                                object.sector
                            }
                        )))
                        .child(
                            div().flex().justify_end().child(
                                button(
                                    SharedString::from(format!("theme-peer-watch-{}", peer.code())),
                                    "+ 盯盘",
                                )
                                .ghost()
                                .on_click(cx.listener(
                                    move |_, _, _, cx| {
                                        cx.emit(DesktopEvent::AddToWatch(watch.clone()))
                                    },
                                )),
                            ),
                        ),
                );
            }
        }
        let mut symbols = object.related.clone();
        for symbol in &object.generic {
            if !symbols.contains(symbol) {
                symbols.push(*symbol);
            }
        }
        let watch = self.context(object, symbols.clone(), cx);
        view.child(
            div()
                .p_3()
                .flex()
                .flex_wrap()
                .gap_2()
                .child(
                    button("theme-copy-related", "复制代码").on_click(cx.listener(
                        move |_, _, _, cx| {
                            cx.write_to_clipboard(ClipboardItem::new_string(
                                symbols
                                    .iter()
                                    .map(|s| s.code())
                                    .collect::<Vec<_>>()
                                    .join("\n"),
                            ));
                            cx.emit(DesktopEvent::Notice("已复制合并去重后的联动代码".into()));
                        },
                    )),
                )
                .child(
                    button("theme-watch-related", "全部加入盯盘")
                        .primary()
                        .on_click(cx.listener(move |_, _, _, cx| {
                            cx.emit(DesktopEvent::AddToWatch(watch.clone()))
                        })),
                ),
        )
        .into_any_element()
    }
}

impl Render for ThemePage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let narrow = window.viewport_size().width < px(1400.);
        let requested = self.requested.read(cx).value().to_string();
        let from = self.from.read(cx).value().to_string();
        let actual = actual_date(&requested);
        let objects = self.objects(cx);
        let object = objects
            .iter()
            .find(|o| o.symbol == self.selected)
            .or_else(|| objects.first());
        let mut object_list = panel()
            .w(px(226.))
            .flex_shrink_0()
            .flex()
            .flex_col()
            .child(section_title("四板观察对象").child(tag(objects.len().to_string(), MUTED)))
            .child(
                div()
                    .p_3()
                    .child(Input::new(&self.query).id("theme-search")),
            );
        for item in &objects {
            let symbol = item.symbol;
            let active = object.is_some_and(|o| o.symbol == symbol);
            object_list = object_list.child(
                div()
                    .id(SharedString::from(format!(
                        "theme-object-{}",
                        symbol.code()
                    )))
                    .test_support()
                    .p_4()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .border_t_1()
                    .border_color(rgb(LINE))
                    .border_l(px(3.))
                    .border_color(rgb(if active { TEAL } else { LINE }))
                    .bg(rgb(if active { 0xeaf7f4 } else { 0xffffff }))
                    .cursor_pointer()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(div().font_weight(FontWeight::SEMIBOLD).child(symbol.name()))
                            .child(tag("4 板", TEAL)),
                    )
                    .child(muted(format!(
                        "{}   {} 触发",
                        symbol.code(),
                        &item.trigger[5..]
                    )))
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(rgb(TEAL))
                            .child(format!("{}  ›", item.sector)),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.selected = symbol;
                        this.related_tab = false;
                        cx.notify();
                    })),
            );
        }
        object_list = object_list.child(
            div()
                .p_4()
                .child(muted("按已导入日计算。漏导资料可能影响周期识别。")),
        );
        let mut content = div()
            .flex_shrink_0()
            .flex()
            .items_start()
            .gap_3()
            .child(object_list);
        if let Some(object) = object {
            if narrow {
                content = content.child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(
                            div()
                                .flex()
                                .gap_2()
                                .child(
                                    button("theme-evidence-view", "周期与证据")
                                        .when(!self.related_tab, |b| b.primary())
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.related_tab = false;
                                            cx.notify();
                                        })),
                                )
                                .child(
                                    button("theme-related-view", "截止日联动")
                                        .when(self.related_tab, |b| b.primary())
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.related_tab = true;
                                            cx.notify();
                                        })),
                                ),
                        )
                        .child(if self.related_tab {
                            self.related(object, cx)
                        } else {
                            self.evidence(object, cx)
                        }),
                );
            } else {
                content = content.child(self.evidence(object, cx)).child(
                    div()
                        .w(px(286.))
                        .flex_shrink_0()
                        .child(self.related(object, cx)),
                );
            }
        } else {
            content = content.child(
                panel()
                    .flex_1()
                    .p_8()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(div().text_size(px(20.)).child("没有符合条件的四板周期"))
                    .child(muted(
                        "扩大日期范围或清除搜索。早期资料不会被替换成未来的四板对象。",
                    ))
                    .child(
                        button("theme-reset-empty", "重置筛选").on_click(cx.listener(
                            |this, _, window, cx| {
                                this.from
                                    .update(cx, |i, cx| i.set_value("2026-09-01", window, cx));
                                this.requested
                                    .update(cx, |i, cx| i.set_value(DATES[0], window, cx));
                                this.query.update(cx, |i, cx| i.set_value("", window, cx));
                                this.incoming = None;
                                cx.notify();
                            },
                        )),
                    ),
            );
        }
        page().id("theme-page").test_support().overflow_y_scroll().gap_3()
            .child(heading("追溯周期 · 理解关联", "题材研究", "对象、证据与截止日联动，放在同一条研究路径上").flex_shrink_0())
            .child(panel().p_3().flex().flex_wrap().items_center().gap_2()
                .child(muted("触发区间"))
                .child(div().w(px(135.)).child(Input::new(&self.from).id("theme-from-date")))
                .child(muted("至"))
                .child(div().w(px(135.)).child(Input::new(&self.requested).id("theme-cutoff-date")))
                .child(tag(format!("实际截止日  {}", actual.unwrap_or("无可用资料")), if actual == Some(requested.as_str()) { TEAL } else { 0xb58a50 }))
                .when(actual.is_some() && actual != Some(requested.as_str()), |view| view.child(muted("已回退至最近导入日")))
                .child(div().flex_1())
                .child(button("theme-reset-date", "重置日期").ghost().on_click(cx.listener(|this, _, window, cx| {
                    this.from.update(cx, |i, cx| i.set_value("2026-09-01", window, cx)); this.requested.update(cx, |i, cx| i.set_value(DATES[0], window, cx)); this.incoming = None; cx.notify();
                }))))
            .when(!valid_date(&from) || !valid_date(&requested) || from > requested, |view| view.child(div().flex_shrink_0().text_color(rgb(0xc92f57)).child("请输入有效日期 YYYY-MM-DD，起始日不能晚于截止日。")))
            .when_some(self.incoming.as_ref(), |view, incoming| view.child(div().flex_shrink_0().px_3().py_2().rounded(px(4.)).bg(rgb(0xeaf7f4)).text_size(px(12.)).text_color(rgb(TEAL)).child(format!("来自涨停资料：{} · {} · {}", incoming.date, incoming.sector, incoming.reason))))
            .child(panel().p_3().flex_shrink_0().flex().flex_wrap().items_center().gap_3()
                .child(div().flex_1().min_w(px(300.)).child(div().font_weight(FontWeight::SEMIBOLD).child("模拟候选演练"))
                    .child(muted("固定样例：10 月 14 日首板东方财富，确认示例题材强势后，生成 10 月 15 日观察草稿。独立于上方资料。")))
                .child(button("theme-demo-candidate", "确认示例强势 · 生成候选草稿").on_click(cx.listener(|_,_,_,cx| {
                    match tydaro_core::research::demonstration_candidates() {
                        Ok(set)=>cx.emit(DesktopEvent::CandidateDraft(set)),
                        Err(error)=>cx.emit(DesktopEvent::Notice(format!("模拟候选未生成：{error}"))),
                    }
                }))))
            .child(content)
            .child(muted("四板对象、梯队与关键词是固定研究样例；日期用于筛选和解释，不代表已实现旧系统计算算法。").flex_shrink_0())
    }
}
