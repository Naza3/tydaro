use super::*;

impl PanelContent {
    pub(super) fn news(&self, cx: &mut Context<Self>) -> AnyElement {
        let items = [
            (
                "公司动态",
                "09:42",
                format!(
                    "{}季度财报观察：关注营收结构与经营现金流",
                    self.symbol.name()
                ),
                "这是一条用于验证资讯排版和联动的本地示例。后续接入数据服务后，正文、发布时间、来源与公司公告将由统一资讯接口提供。",
            ),
            (
                "市场观察",
                "09:18",
                "全球市场晨报：科技板块与主要指数表现".into(),
                "模拟资讯正文：工作区支持同时打开多个股票面板。点击股票列表可以联动图表、报价、盘口和资讯；固定到独立联动组后，可以并排比较不同标的。",
            ),
            (
                "产品说明",
                "08:50",
                "自定义你的工作台：拖动标签即可调整面板位置".into(),
                "将标签拖到其他面板的上、下、左、右边缘可插入分区，拖到中央可合并标签。通过面板菜单弹出独立窗口，布局会保存到本机。",
            ),
            (
                "公司公告",
                "昨天",
                format!("{}投资者关系资料与历史公告入口（示例）", self.symbol.name()),
                "本地演示不会打开外部交易服务，也不会提交真实订单。模拟委托只保存在本次应用会话中，用于检查买卖表单、订单与持仓的交互。",
            ),
        ];
        div()
            .id("news-scroll")
            .size_full()
            .overflow_y_scroll()
            .children(
                items
                    .into_iter()
                    .enumerate()
                    .map(|(index, (tag, time, title, body))| {
                        div()
                            .id(SharedString::from(format!("news-{index}")))
                            .p_3()
                            .border_b_1()
                            .border_color(rgb(LINE))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(0xf8fafc)))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .mb_2()
                                    .child(badge(tag))
                                    .child(
                                        div().text_size(px(12.)).text_color(rgb(MUTED)).child(time),
                                    ),
                            )
                            .child(div().line_height(relative(1.55)).child(title))
                            .when(self.expanded_news == Some(index), |d| {
                                d.child(
                                    div()
                                        .mt_2()
                                        .text_size(px(12.))
                                        .line_height(relative(1.65))
                                        .text_color(rgb(MUTED))
                                        .child(body),
                                )
                            })
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.expanded_news = if this.expanded_news == Some(index) {
                                    None
                                } else {
                                    Some(index)
                                };
                                cx.notify();
                            }))
                            .test_support()
                    }),
            )
            .into_any_element()
    }
}
