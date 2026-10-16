use super::*;

impl WatchPage {
    pub(super) fn sidebar(
        &self,
        groups: &[WatchGroup],
        alerts: &[WatchAlert],
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let unread = alerts.iter().filter(|alert| !alert.read).count();
        let mut navigation = div().flex().flex_col().gap_1().px_2();
        for (index, group) in groups.iter().enumerate() {
            let id = group.id.clone();
            let selected = self.selected_group.as_ref() == Some(&id);
            navigation = navigation.child(
                div()
                    .id(SharedString::from(format!("watch-nav-{id}")))
                    .test_support()
                    .h(px(40.))
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .rounded(px(5.))
                    .cursor_pointer()
                    .bg(rgb(if selected { 0xeaf5f4 } else { 0xffffff }))
                    .hover(|style| style.bg(rgb(0xf3f7f8)))
                    .child(div().size(px(7.)).rounded_full().bg(rgb(group.color)))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .child(group.name.clone()),
                    )
                    .child(small(group.symbols.len().to_string()))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.selected_group = Some(id.clone());
                        this.scroll.scroll_to_item(1 + index / this.actual_columns);
                        cx.notify();
                    })),
            );
        }
        let mut recent = div().flex().flex_col().px_3();
        for alert in alerts.iter().take(3) {
            let selected = alert.clone();
            recent = recent.child(
                div()
                    .id(SharedString::from(format!("watch-recent-{}", alert.id)))
                    .test_support()
                    .py_3()
                    .border_b_1()
                    .border_color(rgb(LINE))
                    .cursor_pointer()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .hover(|style| style.bg(rgb(0xf6f9fa)))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(div().size(px(5.)).rounded_full().bg(rgb(if alert.read {
                                LINE
                            } else {
                                TEAL
                            })))
                            .child(div().flex_1().text_size(px(12.)).child(alert.title.clone()))
                            .child(small(alert.time.clone())),
                    )
                    .child(
                        div()
                            .pl_3()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(alert.symbol.name())
                            .child(small(alert.symbol.code())),
                    )
                    .on_click(
                        cx.listener(move |this, _, _, cx| this.open_alert(selected.clone(), cx)),
                    ),
            );
        }
        if alerts.is_empty() {
            recent = recent.child(
                div()
                    .py_8()
                    .text_center()
                    .child(small("暂无异动，点击演示异动生成")),
            );
        }
        let rule_count = cx
            .global::<DesktopState>()
            .rules
            .iter()
            .filter(|rule| rule.enabled)
            .count();
        panel()
            .w(px(216.))
            .flex_shrink_0()
            .flex()
            .flex_col()
            .py_2()
            .gap_2()
            .child(
                div()
                    .h(px(38.))
                    .flex_shrink_0()
                    .px_3()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(bold("我的分组"))
                    .child(
                        icon_button("watch-add-group-side", IconName::Plus)
                            .tooltip("新建盯盘分组")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.begin_group(None, window, cx)
                            })),
                    ),
            )
            .child(navigation)
            .child(div().mx_3().my_3().h(px(1.)).bg(rgb(LINE)))
            .child(
                div()
                    .px_3()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(bold("最近异动"))
                    .child(tag(format!("{unread} 未读"), TEAL)),
            )
            .child(recent)
            .child(
                div().px_3().child(
                    button("watch-show-alerts", "查看全部异动")
                        .w_full()
                        .icon(IconName::ChevronRight)
                        .on_click(cx.listener(|this, _, _, cx| {
                            let count = cx
                                .global::<DesktopState>()
                                .watch_groups
                                .len()
                                .div_ceil(this.actual_columns);
                            this.scroll.scroll_to_item(count + 1);
                            cx.notify();
                        })),
                ),
            )
            .child(
                div()
                    .px_3()
                    .pt_3()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(small(if cx.global::<DesktopState>().monitoring {
                        "模拟规则已就绪"
                    } else {
                        "已暂停生成异动"
                    }))
                    .child(
                        button("watch-rule-settings", format!("{rule_count} 条规则"))
                            .ghost()
                            .icon(IconName::Settings2)
                            .on_click(
                                cx.listener(|this, _, window, cx| this.begin_rules(window, cx)),
                            ),
                    ),
            )
            .into_any_element()
    }

    pub(super) fn alerts_panel(&self, alerts: &[WatchAlert], cx: &mut Context<Self>) -> AnyElement {
        let unread = alerts.iter().filter(|alert| !alert.read).count();
        let mut body = div().flex().flex_col();
        for alert in alerts
            .iter()
            .filter(|alert| !self.unread_only || !alert.read)
        {
            let item = alert.clone();
            body = body.child(
                div()
                    .id(SharedString::from(format!("watch-alert-{}", alert.id)))
                    .test_support()
                    .h(px(47.))
                    .px_4()
                    .flex()
                    .items_center()
                    .gap_3()
                    .border_b_1()
                    .border_color(rgb(0xeff2f5))
                    .cursor_pointer()
                    .bg(rgb(if alert.read { 0xffffff } else { 0xf8fbfb }))
                    .hover(|style| style.bg(rgb(0xeef6f6)))
                    .child(
                        div()
                            .w(px(6.))
                            .h(px(6.))
                            .rounded_full()
                            .bg(rgb(if alert.read { LINE } else { TEAL })),
                    )
                    .child(
                        div()
                            .w(px(70.))
                            .text_size(px(12.))
                            .text_color(rgb(MUTED))
                            .child(alert.time.clone()),
                    )
                    .child(div().w(px(100.)).child(alert.symbol.name()))
                    .child(div().w(px(90.)).child(tag(alert.title.clone(), TEAL)))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(12.))
                            .truncate()
                            .child(alert.detail.clone()),
                    )
                    .child(
                        div()
                            .w(px(132.))
                            .text_size(px(12.))
                            .text_color(rgb(MUTED))
                            .truncate()
                            .child(alert.group_name.clone()),
                    )
                    .child(
                        Icon::new(IconName::ChevronRight)
                            .size(px(14.))
                            .text_color(rgb(MUTED)),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| this.open_alert(item.clone(), cx))),
            );
        }
        if alerts.is_empty() || (self.unread_only && unread == 0) {
            body = body.child(
                div()
                    .h(px(100.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(small(if self.unread_only {
                        "未读异动已处理完毕"
                    } else {
                        "暂无异动 · 点击“演示异动”查看规则效果"
                    })),
            );
        }
        panel().id("watch-alerts").test_support().overflow_hidden()
            .child(div().h(px(52.)).px_4().flex().items_center().gap_3()
                .child(Icon::new(IconName::Bell).size(px(16.)).text_color(rgb(MUTED)))
                .child(bold("异动记录")).child(tag(format!("{unread} 条未读"), TEAL))
                .child(div().flex_1().child(small("最新在上 · 最多 100 条")))
                .child(button("watch-demo-alert", "演示异动").icon(IconName::Activity).disabled(!cx.global::<DesktopState>().monitoring).on_click(cx.listener(|this, _, _, cx| this.demo_alert(cx))))
                .child(icon_button("watch-unread-only", IconName::ListFilter).tooltip(if self.unread_only { "查看全部异动" } else { "只看未读异动" }).text_color(rgb(if self.unread_only { TEAL } else { MUTED })).on_click(cx.listener(|this, _, _, cx| { this.unread_only = !this.unread_only; cx.notify(); })))
                .child(icon_button("watch-alerts-read", IconName::CheckCheck).tooltip("全部标为已读").disabled(unread == 0).on_click(cx.listener(|this, _, _, cx| this.simple_command(Command::ReadAllAlerts,cx))))
                .child(icon_button("watch-alerts-clear", IconName::Trash).tooltip("清除已读记录").disabled(!alerts.iter().any(|alert| alert.read)).on_click(cx.listener(|this, _, _, cx| this.simple_command(Command::ClearReadAlerts,cx)))))
            .child(div().h(px(28.)).px_4().flex().items_center().gap_3().bg(rgb(0xf8fafb)).text_size(px(12.)).text_color(rgb(MUTED))
                .child(div().w(px(6.))).child(div().w(px(70.)).child("时间")).child(div().w(px(100.)).child("股票")).child(div().w(px(90.)).child("触发规则")).child(div().flex_1().child("规则详情 / 演示测量值")).child(div().w(px(158.)).child("所属分组")))
            .child(body)
            .child(div().px_4().py_3().child(small("仅展示本地模拟事件；静音开关保存偏好，本阶段不播放音频。月热点为研究样例，非已完成的历史计算。")))
            .into_any_element()
    }
}
