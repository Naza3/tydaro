//! Native display primitives for the component gallery. All state stays in this view.
use gpui_kit::{
    component::{
        Disableable, Sizable, WindowExt,
        button::{Button, ButtonVariants},
    },
    prelude::*,
    *,
};

use tydaro_ui::{selection, tokens::COOL_LIGHT};

use crate::gallery::foundation::{
    ACCENT, DOWN, Family, INK, LINE, SOFT, SURFACE, UP, Variant, action, caption, hint, surface,
};

const ROWS: [(&str, &str, &str, &str, u32); 3] = [
    ("中际旭创", "300308", "162.80", "+2.46%", UP),
    ("东方财富", "300059", "18.46", "−0.72%", DOWN),
    ("中芯国际", "688981", "52.30", "+1.16%", UP),
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum FeedbackStage {
    Ready,
    Failed,
    Done,
}

pub struct DisplaySamples {
    variant: Variant,
    family: Family,
    selected: usize,
    disclosure_open: bool,
    feedback: FeedbackStage,
    feedback_note: String,
}

impl DisplaySamples {
    pub fn new(variant: Variant, _window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self {
            variant,
            family: Family::List,
            selected: 0,
            disclosure_open: false,
            feedback: FeedbackStage::Ready,
            feedback_note: String::new(),
        }
    }

    pub fn set_family(&mut self, family: Family, cx: &mut Context<Self>) {
        self.family = family;
        cx.notify();
    }

    #[cfg(feature = "test-support")]
    pub fn snapshot(&self) -> (usize, bool, usize) {
        (
            self.selected,
            self.disclosure_open,
            if self.feedback == FeedbackStage::Done {
                0
            } else {
                3
            },
        )
    }

    fn id(&self, name: &str) -> SharedString {
        format!("gallery-{}-{name}", self.variant.key()).into()
    }

    fn row_button(&self, index: usize, cx: &mut Context<Self>) -> Button {
        selection::list_row(
            self.id(&format!("list-row-{index}")),
            self.selected == index,
            COOL_LIGHT,
        )
        .on_click(cx.listener(move |this, _, _, cx| {
            this.selected = index;
            cx.notify();
        }))
    }

    fn selected_note(&self) -> AnyElement {
        hint(format!("已选择 {} · 点击另一行切换", ROWS[self.selected].0))
            .id(self.id("list-selection"))
            .test_support()
            .into_any_element()
    }

    fn list(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut body = div().flex().flex_col().gap_3().min_w_0();
        match self.variant {
            Variant::Precision => {
                let mut table = surface(self.variant).flex().flex_col();
                table = table.child(
                    div()
                        .flex()
                        .items_center()
                        .h(px(30.))
                        .px_3()
                        .bg(rgb(SOFT))
                        .border_b_1()
                        .border_color(rgb(LINE))
                        .child(hint("名称 / 代码").flex_1())
                        .child(hint("现价").w(px(62.)).text_right())
                        .child(hint("涨跌").w(px(68.)).text_right()),
                );
                for (index, &(name, code, price, change, color)) in ROWS.iter().enumerate() {
                    table = table.child(
                        self.row_button(index, cx)
                            .h(px(52.))
                            .rounded_none()
                            .px_3()
                            .border_b_1()
                            .border_color(rgb(LINE))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .w_full()
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .flex()
                                            .flex_col()
                                            .items_start()
                                            .child(caption(name))
                                            .child(hint(code).text_size(px(11.))),
                                    )
                                    .child(number(price, INK).w(px(62.)).text_right())
                                    .child(number(change, color).w(px(68.)).text_right()),
                            ),
                    );
                }
                body = body
                    .child(table)
                    .child(self.selected_note())
                    .child(hint("固定列位 · 数字右对齐 · 同屏比较"));
            }
            Variant::Calm => {
                let mut list = div().flex().flex_col().gap_2();
                for (index, &(name, code, price, change, color)) in ROWS.iter().enumerate() {
                    let selected = self.selected == index;
                    list = list.child(
                        self.row_button(index, cx)
                            .h(px(74.))
                            .px_4()
                            .rounded_lg()
                            .border_1()
                            .border_l(px(3.))
                            .border_color(rgb(if selected { ACCENT } else { LINE }))
                            .bg(rgb(if selected { SOFT } else { SURFACE }))
                            .child(
                                div()
                                    .w_full()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .gap_3()
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .items_start()
                                            .gap_1()
                                            .child(caption(name))
                                            .child(hint(format!("{code} · 模拟"))),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .items_end()
                                            .gap_1()
                                            .child(number(price, INK).text_size(px(17.)))
                                            .child(number(change, color).text_size(px(12.))),
                                    ),
                            ),
                    );
                }
                body = body
                    .child(list)
                    .child(self.selected_note())
                    .child(hint("两行信息 · 留白分组 · 持续显示选中边线"));
            }
            Variant::Context => {
                let mut list = div().flex().flex_col().border_b_1().border_color(rgb(LINE));
                for (index, &(name, code, _, _, _)) in ROWS.iter().enumerate() {
                    list = list.child(
                        self.row_button(index, cx)
                            .h(px(42.))
                            .rounded_none()
                            .px_3()
                            .child(
                                div()
                                    .w_full()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(div().w(px(5.)).h(px(5.)).rounded_full().bg(rgb(
                                        if self.selected == index { ACCENT } else { LINE },
                                    )))
                                    .child(caption(name).flex_1())
                                    .child(hint(code))
                                    .child(hint("›").text_size(px(18.))),
                            ),
                    );
                }
                let (name, code, price, change, color) = ROWS[self.selected];
                let inspector = div()
                    .id(self.id("list-inspector"))
                    .test_support()
                    .p_4()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .bg(rgb(SOFT))
                    .child(hint("当前条目 / INSPECTOR").text_size(px(10.)))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(caption(name).text_size(px(16.)))
                            .child(hint(code)),
                    )
                    .child(
                        div()
                            .flex()
                            .items_baseline()
                            .gap_3()
                            .child(number(price, INK).text_size(px(27.)))
                            .child(number(change, color)),
                    )
                    .child(hint("只展开当前项的细节，其他条目保留在上方。"));
                body = body
                    .child(surface(self.variant).child(list).child(inspector))
                    .child(hint("精简目录 + 单条检视 · 减少重复字段"));
            }
        }
        body.into_any_element()
    }

    fn parameters(&self, detailed: bool) -> Div {
        let mut fields = div().flex().flex_col().gap_3();
        for (label, value, detail) in [
            ("观察窗口", "20 个交易日", "使用相同时间范围比较样本"),
            ("触发阈值", "+2.00%", "达到阈值才显示演示提示"),
            ("重复提醒", "仅首次", "同一条目只保留一次提示"),
        ] {
            fields = fields.child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_2()
                            .child(hint(label))
                            .child(caption(value)),
                    )
                    .when(detailed, |this| this.child(hint(detail).text_size(px(11.)))),
            );
        }
        fields
    }

    fn disclosure(&self, cx: &mut Context<Self>) -> AnyElement {
        let opened = self.disclosure_open;
        let mut body = div().flex().flex_col().gap_4();
        match self.variant {
            Variant::Precision => {
                body = body
                    .child(
                        surface(self.variant)
                            .child(
                                self.disclosure_toggle(cx).h(px(43.)).rounded_none().child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .w_full()
                                        .child(hint(if opened { "⌄" } else { "›" }))
                                        .child(caption("提醒规则").flex_1())
                                        .child(hint("3 项")),
                                ),
                            )
                            .when(opened, |this| {
                                this.child(
                                    div()
                                        .id(self.id("disclosure-content"))
                                        .test_support()
                                        .p_3()
                                        .border_t_1()
                                        .border_color(rgb(LINE))
                                        .child(self.parameters(false)),
                                )
                            }),
                    )
                    .child(hint("箭头控制原位展开，适合密集参数区。"));
            }
            Variant::Calm => {
                body = body
                    .child(
                        surface(self.variant)
                            .p_4()
                            .flex()
                            .flex_col()
                            .gap_4()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(caption("提醒规则"))
                                    .child(hint("20 日窗口 · +2.00% · 仅首次")),
                            )
                            .child(
                                self.disclosure_toggle(cx)
                                    .label(if opened {
                                        "收起完整参数 ↑"
                                    } else {
                                        "查看完整参数 ↓"
                                    })
                                    .border_1()
                                    .border_color(rgb(LINE)),
                            )
                            .when(opened, |this| {
                                this.child(
                                    div()
                                        .id(self.id("disclosure-content"))
                                        .test_support()
                                        .pt_4()
                                        .border_t_1()
                                        .border_color(rgb(LINE))
                                        .child(self.parameters(true)),
                                )
                            }),
                    )
                    .child(hint("先读一句摘要，需要时再展开说明。"));
            }
            Variant::Context => {
                body = body.child(
                    surface(self.variant)
                        .p_3()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .flex_1()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(caption("提醒规则"))
                                .child(hint("3 项配置 · 只读示例")),
                        )
                        .child(
                            action(self.id("disclosure-toggle"), self.variant, "检视 →")
                                .disabled(opened)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.disclosure_open = true;
                                    cx.notify();
                                })),
                        ),
                );
                if opened {
                    body = body.child(
                        div()
                            .id(self.id("disclosure-content"))
                            .test_support()
                            .p_4()
                            .border_l(px(2.))
                            .border_color(rgb(ACCENT))
                            .bg(rgb(SOFT))
                            .flex()
                            .flex_col()
                            .gap_4()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .child(caption("规则检视"))
                                    .child(
                                        action(self.id("disclosure-close"), self.variant, "关闭 ×")
                                            .ghost()
                                            .small()
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.disclosure_open = false;
                                                cx.notify();
                                            })),
                                    ),
                            )
                            .child(self.parameters(true)),
                    );
                } else {
                    body = body.child(
                        div()
                            .p_4()
                            .border_l(px(2.))
                            .border_color(rgb(LINE))
                            .child(hint("检视时在此呈现完整规则，上方摘要保留。")),
                    );
                }
                body = body.child(hint("独立检视区 · 显式关闭 · 摘要始终可见"));
            }
        }
        body.into_any_element()
    }

    fn disclosure_toggle(&self, cx: &mut Context<Self>) -> Button {
        Button::new(self.id("disclosure-toggle"))
            .ghost()
            .w_full()
            .justify_start()
            .px_3()
            .text_size(px(13.))
            .on_click(cx.listener(|this, _, _, cx| {
                this.disclosure_open = !this.disclosure_open;
                cx.notify();
            }))
    }

    fn clear(&mut self, cx: &mut Context<Self>) {
        self.feedback = FeedbackStage::Done;
        self.feedback_note = "3 条演示记录已清除".into();
        cx.notify();
    }

    fn restore(&mut self, cx: &mut Context<Self>) {
        self.feedback = FeedbackStage::Ready;
        self.feedback_note = "已恢复 3 条演示记录".into();
        cx.notify();
    }

    fn open_clear_confirmation(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let owner = cx.weak_entity();
        let confirmed = owner.clone();
        let canceled = owner.clone();
        let confirm_id = self.id("feedback-confirm");
        let cancel_id = self.id("feedback-cancel");
        window.open_dialog(cx, move |dialog, _, _| {
            let confirmed = confirmed.clone();
            let canceled = canceled.clone();
            dialog.title("清除 3 条演示记录？").width(px(420.)).child(
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .text_color(rgb(INK))
                    .child("这次操作只改变当前 Gallery 中的样例状态。")
                    .child(hint("股票列表、研究资料与个人布局均不受影响。"))
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .gap_2()
                            .child(Button::new(cancel_id.clone()).label("保留记录").on_click(
                                move |_, window, cx| {
                                    let _ = canceled.update(cx, |this, cx| {
                                        this.feedback_note = "已取消，保留 3 条演示记录".into();
                                        cx.notify();
                                    });
                                    window.close_dialog(cx);
                                },
                            ))
                            .child(
                                Button::new(confirm_id.clone())
                                    .label("确认清除")
                                    .primary()
                                    .on_click(move |_, window, cx| {
                                        let _ = confirmed.update(cx, |this, cx| this.clear(cx));
                                        window.close_dialog(cx);
                                    }),
                            ),
                    ),
            )
        });
    }

    fn feedback(&self, cx: &mut Context<Self>) -> AnyElement {
        let done = self.feedback == FeedbackStage::Done;
        let mut body = div().flex().flex_col().gap_4().child(
            surface(self.variant)
                .p_4()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(caption("演示记录"))
                        .child(hint("仅当前组件的临时状态")),
                )
                .child(
                    number(if done { "0" } else { "3" }, INK)
                        .id(self.id("feedback-count"))
                        .test_support()
                        .text_size(px(30.)),
                ),
        );
        match self.variant {
            Variant::Precision => {
                body = body.child(
                    action(self.id("feedback-run"), self.variant, "清除演示记录")
                        .disabled(done || self.feedback == FeedbackStage::Failed)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.feedback = FeedbackStage::Failed;
                            this.feedback_note = "演示：首次操作失败，记录仍保留".into();
                            cx.notify();
                        })),
                );
                if self.feedback == FeedbackStage::Failed {
                    body = body.child(
                        div()
                            .p_3()
                            .border_l(px(2.))
                            .border_color(rgb(UP))
                            .bg(rgb(SOFT))
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(caption("未完成 · 3 条记录仍保留"))
                            .child(hint("演示一次失败，可在原位置重试。"))
                            .child(
                                action(self.id("feedback-retry"), self.variant, "重新尝试")
                                    .on_click(cx.listener(|this, _, _, cx| this.clear(cx))),
                            ),
                    );
                } else if done {
                    body = body.child(
                        div()
                            .p_3()
                            .border_l(px(2.))
                            .border_color(rgb(ACCENT))
                            .bg(rgb(SOFT))
                            .child(caption("✓ 操作完成 · 已清除 3 条")),
                    );
                }
                body = body.child(hint("原位反馈：失败保留内容，重试后更新结果。"));
            }
            Variant::Calm => {
                if done {
                    body = body.child(
                        surface(self.variant)
                            .p_4()
                            .flex()
                            .flex_col()
                            .gap_3()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .size(px(26.))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .rounded_full()
                                            .bg(rgb(SOFT))
                                            .text_color(rgb(ACCENT))
                                            .child("✓"),
                                    )
                                    .child(caption("已清除 3 条记录")),
                            )
                            .child(hint("回执保留，撤销即可完整恢复。"))
                            .child(
                                action(self.id("feedback-undo"), self.variant, "撤销这次操作")
                                    .on_click(cx.listener(|this, _, _, cx| this.restore(cx))),
                            ),
                    );
                } else {
                    body = body.child(
                        action(self.id("feedback-run"), self.variant, "清除演示记录")
                            .on_click(cx.listener(|this, _, _, cx| this.clear(cx))),
                    );
                }
                body = body.child(hint("先执行、后撤销：用持续回执承接可逆操作。"));
            }
            Variant::Context => {
                body = body
                    .child(
                        div()
                            .p_3()
                            .border_l(px(2.))
                            .border_color(rgb(LINE))
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(caption(if done {
                                "操作已确认"
                            } else {
                                "先检查，再执行"
                            }))
                            .child(hint(if done {
                                "确认后已清除记录，可恢复样例继续比较。"
                            } else {
                                "打开对话框查看影响范围，取消会保留全部记录。"
                            })),
                    )
                    .child(
                        action(self.id("feedback-run"), self.variant, "检查并清除…")
                            .disabled(done)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.open_clear_confirmation(window, cx);
                            })),
                    )
                    .child(hint("确认对话框：显式范围、明确取消、确认后执行。"));
            }
        }
        if !self.feedback_note.is_empty() {
            body = body.child(
                hint(self.feedback_note.clone())
                    .id(self.id("feedback-result"))
                    .test_support(),
            );
        }
        if done && self.variant != Variant::Calm {
            body = body.child(
                action(self.id("feedback-restore"), self.variant, "恢复样例")
                    .ghost()
                    .on_click(cx.listener(|this, _, _, cx| this.restore(cx))),
            );
        }
        body.into_any_element()
    }
}

impl Render for DisplaySamples {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .w_full()
            .min_w_0()
            .text_color(rgb(INK))
            .text_size(px(13.))
            .child(match self.family {
                Family::List => self.list(cx),
                Family::Disclosure => self.disclosure(cx),
                Family::Feedback => self.feedback(cx),
                _ => div().into_any_element(),
            })
    }
}

fn number(text: impl Into<SharedString>, color: u32) -> Div {
    tydaro_ui::text::number(text, color)
}
