//! Independent list membership, directory browsing and import entry points.
use super::*;

mod import;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum StockListPage {
    List,
    Catalog,
    Import,
}

impl PanelContent {
    pub(super) fn show_stock_catalog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.stock_list_page = StockListPage::Catalog;
        self.catalog_query().update(cx, |input, cx| {
            input.set_value("", window, cx);
            input.focus(window, cx);
        });
        cx.notify();
    }

    pub(super) fn stock_catalog(&self, cx: &mut Context<Self>) -> AnyElement {
        let query = self.catalog_query().read(cx).value().trim().to_lowercase();
        let symbols: Vec<_> = Symbol::ALL
            .into_iter()
            .filter(|symbol| {
                symbol.code().to_lowercase().contains(&query) || symbol.name().contains(&query)
            })
            .collect();
        let empty = symbols.is_empty();
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .px_2()
                    .pt_1()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(div().font_weight(FontWeight::MEDIUM).child("添加股票"))
                    .child(
                        Button::new("stock-list-back")
                            .label("返回列表")
                            .small()
                            .ghost()
                            .text_color(rgb(TEAL))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.stock_list_page = StockListPage::List;
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div().p_2().child(
                    Input::new(self.catalog_query())
                        .id("stock-list-catalog-search")
                        .small()
                        .w_full(),
                ),
            )
            .child(
                Button::new("stock-catalog-import")
                    .label("从文本导入")
                    .small()
                    .ghost()
                    .text_color(rgb(TEAL))
                    .on_click(
                        cx.listener(|this, _, window, cx| this.show_stock_import(window, cx)),
                    ),
            )
            .child(
                div()
                    .id("stock-list-catalog-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .children(symbols.into_iter().map(|symbol| {
                        let exists = self.members.contains(&symbol);
                        div()
                            .h(px(48.))
                            .px_2()
                            .flex()
                            .items_center()
                            .gap_2()
                            .border_b_1()
                            .border_color(rgb(LINE))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(div().truncate().child(symbol.name()))
                                    .child(
                                        div()
                                            .truncate()
                                            .text_size(px(12.))
                                            .text_color(rgb(MUTED))
                                            .child(format!(
                                                "{}  {}",
                                                symbol.market(),
                                                symbol.code()
                                            )),
                                    ),
                            )
                            .child(
                                Button::new(SharedString::from(format!(
                                    "stock-list-add-{}",
                                    symbol.code()
                                )))
                                .label(if exists { "已添加" } else { "添加" })
                                .small()
                                .ghost()
                                .flex_shrink_0()
                                .text_color(rgb(TEAL))
                                .disabled(exists)
                                .on_click(cx.listener(
                                    move |this, _, _, cx| {
                                        if !this.members.contains(&symbol) {
                                            this.members.push(symbol);
                                            cx.emit(ContentEvent::ListChanged);
                                            cx.notify();
                                        }
                                    },
                                )),
                            )
                    }))
                    .when(empty, |d| {
                        d.child(
                            div()
                                .p_4()
                                .text_color(rgb(MUTED))
                                .child("模拟股票目录中没有匹配结果"),
                        )
                    }),
            )
            .child(
                div()
                    .px_2()
                    .py_2()
                    .border_t_1()
                    .border_color(rgb(LINE))
                    .text_size(px(12.))
                    .text_color(rgb(MUTED))
                    .child(format!("模拟股票目录 · 当前列表 {} 支", self.members.len())),
            )
            .into_any_element()
    }

    pub(super) fn watchlist(&self, cx: &mut Context<Self>) -> AnyElement {
        match self.stock_list_page {
            StockListPage::Catalog => return self.stock_catalog(cx),
            StockListPage::Import => return self.stock_import_view(cx),
            StockListPage::List => {}
        }
        let query = self.query().read(cx).value().trim().to_lowercase();
        let mut symbols: Vec<_> = self
            .members
            .iter()
            .copied()
            .filter(|s| s.code().to_lowercase().contains(&query) || s.name().contains(&query))
            .collect();
        if self.sort > 0 {
            symbols.sort_by(|a, b| {
                market::quote(*a, self.tick)
                    .change_pct
                    .total_cmp(&market::quote(*b, self.tick).change_pct)
            });
            if self.sort == 1 {
                symbols.reverse();
            }
        }
        let count = symbols.len();
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .p_2()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(
                        div().flex_1().min_w_0().child(
                            Input::new(self.query())
                                .id("watchlist-search")
                                .small()
                                .w_full(),
                        ),
                    )
                    .child(
                        Button::new("stock-list-add")
                            .label("添加")
                            .tooltip("添加股票到当前列表")
                            .small()
                            .ghost()
                            .flex_shrink_0()
                            .text_color(rgb(TEAL))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.show_stock_catalog(window, cx);
                            })),
                    ),
            )
            .child(
                div()
                    .px_2()
                    .pb_1()
                    .flex()
                    .justify_end()
                    .flex_shrink_0()
                    .child(
                        Button::new("stock-list-import")
                            .label("从文本导入")
                            .small()
                            .ghost()
                            .text_color(rgb(TEAL))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.show_stock_import(window, cx)
                            })),
                    ),
            )
            .child(
                div()
                    .h(px(29.))
                    .px_2()
                    .flex()
                    .items_center()
                    .border_b_1()
                    .border_color(rgb(LINE))
                    .text_color(rgb(MUTED))
                    .child(div().flex_1().min_w_0().truncate().child("名称 / 代码"))
                    .child(
                        div()
                            .w(px(58.))
                            .flex_shrink_0()
                            .text_right()
                            .child("最新价"),
                    )
                    .child(
                        control(
                            "watchlist-sort",
                            match self.sort {
                                1 => "涨跌幅 ↓",
                                2 => "涨跌幅 ↑",
                                _ => "涨跌幅 ↕",
                            },
                            false,
                        )
                        .w(px(58.))
                        .px_0()
                        .flex_shrink_0()
                        .text_right()
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.sort = (this.sort + 1) % 3;
                            cx.notify();
                        }))
                        .test_support(),
                    )
                    .child(div().w(px(26.)).flex_shrink_0()),
            )
            .child(
                div()
                    .id("watchlist-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .children(symbols.into_iter().map(|symbol| {
                        let quote = market::quote(symbol, self.tick);
                        div()
                            .id(SharedString::from(format!("symbol-{}", symbol.code())))
                            .h(px(48.))
                            .px_2()
                            .flex()
                            .items_center()
                            .border_b_1()
                            .border_color(rgb(0xf0f2f5))
                            .bg(rgb(if self.symbol == symbol {
                                0xe9f7f7
                            } else {
                                0xffffff
                            }))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(0xf1f8f9)))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(
                                        div()
                                            .truncate()
                                            .text_size(px(14.))
                                            .font_weight(FontWeight::MEDIUM)
                                            .child(symbol.name()),
                                    )
                                    .child(
                                        div()
                                            .truncate()
                                            .text_size(px(12.))
                                            .text_color(rgb(MUTED))
                                            .child(format!(
                                                "{}  {}",
                                                symbol.market(),
                                                symbol.code()
                                            )),
                                    ),
                            )
                            .child(
                                div()
                                    .w(px(58.))
                                    .flex_shrink_0()
                                    .text_right()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(movement(quote.change))
                                    .child(format!("{:.3}", quote.price)),
                            )
                            .child(
                                div()
                                    .w(px(58.))
                                    .flex_shrink_0()
                                    .text_right()
                                    .text_color(movement(quote.change))
                                    .child(format!("{:+.2}%", quote.change_pct)),
                            )
                            .child(
                                Button::new(SharedString::from(format!(
                                    "stock-list-remove-{}",
                                    symbol.code()
                                )))
                                .label("×")
                                .tooltip(format!("从当前列表移除 {}", symbol.code()))
                                .small()
                                .ghost()
                                .w(px(26.))
                                .px_0()
                                .flex_shrink_0()
                                .text_color(rgb(MUTED))
                                .on_click(cx.listener(
                                    move |this, _, _, cx| {
                                        cx.stop_propagation();
                                        this.members.retain(|member| *member != symbol);
                                        cx.emit(ContentEvent::ListChanged);
                                        cx.notify();
                                    },
                                )),
                            )
                            .on_click(
                                cx.listener(move |this, _, _, cx| this.select_symbol(symbol, cx)),
                            )
                            .test_support()
                    }))
                    .when(count == 0, |d| {
                        d.child(
                            div()
                                .p_4()
                                .flex()
                                .flex_col()
                                .items_center()
                                .gap_2()
                                .text_color(rgb(MUTED))
                                .child(if self.members.is_empty() {
                                    "列表为空，添加股票开始关注"
                                } else {
                                    "列表中没有匹配的股票"
                                })
                                .when(self.members.is_empty(), |d| {
                                    d.child(
                                        Button::new("stock-list-empty-add")
                                            .label("添加股票")
                                            .small()
                                            .ghost()
                                            .text_color(rgb(TEAL))
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.show_stock_catalog(window, cx);
                                            })),
                                    )
                                }),
                        )
                    }),
            )
            .child(
                div()
                    .px_3()
                    .py_2()
                    .text_size(px(12.))
                    .text_color(rgb(MUTED))
                    .border_t_1()
                    .border_color(rgb(LINE))
                    .child(if query.is_empty() {
                        format!("{} 支股票 · 点击联动同组组件", self.members.len())
                    } else {
                        format!("显示 {} / {} 支股票", count, self.members.len())
                    }),
            )
            .into_any_element()
    }
}
