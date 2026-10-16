//! Local, independent selection primitives for the native component gallery.
//!
//! All three variants use the same data and color vocabulary. Their differences
//! are composition, target size, and how much information is exposed at once.
use crate::ui_foundation::{
    ACCENT, BG, Family, INK, LINE, MUTED, SOFT, SURFACE, Variant, action, caption, hint, surface,
};
use gpui_kit::{
    component::{
        Selectable, Sizable,
        button::{Button, ButtonVariants},
        checkbox::Checkbox,
        switch::Switch,
    },
    prelude::*,
    *,
};

const CHOICES: [&str; 3] = ["标准", "紧凑", "专注"];
const CHOICE_NOTES: [&str; 3] = [
    "信息与留白保持平衡",
    "缩短行高，容纳更多记录",
    "只突出名称和关键变化",
];
const TAB_LABELS: [&str; 3] = ["概览", "证据", "备注"];
const FILTERS: [&str; 3] = ["已确认", "待观察", "有备注"];

pub struct SelectionSamples {
    variant: Variant,
    family: Family,
    choice: usize,
    choice_open: bool,
    enabled: bool,
    tab: usize,
    filters: [bool; 3],
}

impl SelectionSamples {
    pub fn new(variant: Variant, _: &mut Window, _: &mut Context<Self>) -> Self {
        Self {
            variant,
            family: Family::Choice,
            choice: 0,
            choice_open: false,
            enabled: true,
            tab: 0,
            filters: [true, false, true],
        }
    }

    pub fn set_family(&mut self, family: Family, cx: &mut Context<Self>) {
        self.family = family;
        self.choice_open = false;
        cx.notify();
    }

    #[cfg(feature = "test-support")]
    pub fn snapshot(&self) -> (usize, bool, usize, [bool; 3]) {
        (self.choice, self.enabled, self.tab, self.filters)
    }

    fn id(&self, suffix: &str) -> SharedString {
        format!("gallery-{}-{suffix}", self.variant.key()).into()
    }

    fn body(&self) -> Div {
        div()
            .w_full()
            .flex()
            .flex_col()
            .gap_4()
            .text_color(rgb(INK))
    }

    fn status(&self, label: &str, value: impl Into<SharedString>) -> Div {
        div()
            .w_full()
            .flex()
            .items_center()
            .justify_between()
            .gap_2()
            .pt_3()
            .border_t_1()
            .border_color(rgb(LINE))
            .child(hint(label.to_owned()))
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(rgb(ACCENT))
                    .child(value.into()),
            )
    }

    fn choice_preview(&self) -> Div {
        let rows = [
            ("银力科技", "002731", "+3.62%"),
            ("星河设备", "603218", "+2.18%"),
            ("远景材料", "300682", "+1.05%"),
        ];
        let row_height = match self.choice {
            1 => 28.,
            2 => 46.,
            _ => 38.,
        };
        div()
            .w_full()
            .border_1()
            .border_color(rgb(LINE))
            .rounded_md()
            .overflow_hidden()
            .child(
                div()
                    .px_3()
                    .py_2()
                    .bg(rgb(BG))
                    .child(hint("效果预览 · 模拟记录")),
            )
            .children(rows.into_iter().map(|(name, code, change)| {
                div()
                    .h(px(row_height))
                    .px_3()
                    .flex()
                    .items_center()
                    .justify_between()
                    .border_t_1()
                    .border_color(rgb(LINE))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(div().text_size(px(13.)).child(name))
                            .when(self.choice != 2, |row| row.child(hint(code))),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(rgb(MUTED))
                            .child(change),
                    )
            }))
    }

    fn render_choice(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut body = self
            .body()
            .child(caption("信息密度"))
            .child(hint("同一组选择，三种呈现方式。"));
        match self.variant {
            Variant::Precision => {
                body = body.child(
                    div()
                        .flex()
                        .w_full()
                        .gap_1()
                        .p_1()
                        .rounded_md()
                        .bg(rgb(BG))
                        .children(CHOICES.into_iter().enumerate().map(|(index, label)| {
                            action(self.id(&format!("choice-{index}")), self.variant, label)
                                .flex_1()
                                .h(px(34.))
                                .selected(self.choice == index)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.choice = index;
                                    cx.notify();
                                }))
                        })),
                );
            }
            Variant::Calm => {
                body = body.child(div().flex().flex_col().gap_2().children(
                    CHOICES.into_iter().enumerate().map(|(index, label)| {
                        let selected = self.choice == index;
                        Button::new(self.id(&format!("choice-{index}")))
                            .ghost()
                            .w_full()
                            .h(px(63.))
                            .px_3()
                            .border_1()
                            .border_color(rgb(if selected { ACCENT } else { LINE }))
                            .bg(rgb(if selected { SOFT } else { SURFACE }))
                            .rounded_md()
                            .child(
                                div()
                                    .w_full()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .gap_2()
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .items_start()
                                            .gap_1()
                                            .child(caption(label))
                                            .child(hint(CHOICE_NOTES[index])),
                                    )
                                    .child(
                                        div()
                                            .size(px(16.))
                                            .border_1()
                                            .border_color(rgb(if selected { ACCENT } else { LINE }))
                                            .rounded_full()
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .when(selected, |dot| {
                                                dot.child(
                                                    div()
                                                        .size(px(8.))
                                                        .rounded_full()
                                                        .bg(rgb(ACCENT)),
                                                )
                                            }),
                                    ),
                            )
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.choice = index;
                                cx.notify();
                            }))
                    }),
                ));
            }
            Variant::Context => {
                body = body.child(
                    Button::new(self.id("choice-trigger"))
                        .ghost()
                        .w_full()
                        .h(px(50.))
                        .px_3()
                        .border_1()
                        .border_color(rgb(if self.choice_open { ACCENT } else { LINE }))
                        .rounded_md()
                        .child(
                            div()
                                .w_full()
                                .flex()
                                .items_center()
                                .justify_between()
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .items_start()
                                        .gap_1()
                                        .child(hint("当前选择"))
                                        .child(caption(CHOICES[self.choice])),
                                )
                                .child(if self.choice_open {
                                    "收起 −"
                                } else {
                                    "更改 +"
                                }),
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.choice_open = !this.choice_open;
                            cx.notify();
                        })),
                );
                if self.choice_open {
                    body = body.child(
                        div()
                            .p_1()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .border_1()
                            .border_color(rgb(LINE))
                            .rounded_md()
                            .children(CHOICES.into_iter().enumerate().map(|(index, label)| {
                                Button::new(self.id(&format!("choice-{index}")))
                                    .ghost()
                                    .w_full()
                                    .h(px(45.))
                                    .selected(self.choice == index)
                                    .child(
                                        div()
                                            .w_full()
                                            .flex()
                                            .items_center()
                                            .justify_between()
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_col()
                                                    .items_start()
                                                    .gap_1()
                                                    .child(caption(label))
                                                    .child(hint(CHOICE_NOTES[index])),
                                            )
                                            .child(if self.choice == index { "✓" } else { "" }),
                                    )
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.choice = index;
                                        this.choice_open = false;
                                        cx.notify();
                                    }))
                            })),
                    );
                }
            }
        }
        body.when(
            self.variant != Variant::Context || !self.choice_open,
            |body| body.child(self.choice_preview()),
        )
        .child(self.status("已应用", CHOICES[self.choice]))
        .into_any_element()
    }

    fn render_toggle(&self, cx: &mut Context<Self>) -> AnyElement {
        let setting_description = "控制本列的演示提醒，不影响盯盘设置。";
        let mut body = self.body();
        match self.variant {
            Variant::Precision => {
                body = body.child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_3()
                        .py_3()
                        .border_b_1()
                        .border_color(rgb(LINE))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(caption("演示提醒"))
                                .child(hint("切换后立即生效")),
                        )
                        .child(
                            Switch::new(self.id("toggle"))
                                .checked(self.enabled)
                                .color(rgb(ACCENT))
                                .accessibility_label("演示提醒")
                                .tooltip(if self.enabled {
                                    "关闭演示提醒"
                                } else {
                                    "开启演示提醒"
                                })
                                .on_change(cx.listener(|this, checked, _, cx| {
                                    this.enabled = *checked;
                                    cx.notify();
                                })),
                        ),
                );
            }
            Variant::Calm => {
                body = body.child(
                    surface(self.variant).child(
                        Checkbox::new(self.id("toggle"))
                            .checked(self.enabled)
                            .w_full()
                            .p_4()
                            .accessibility_label("启用演示提醒")
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_2()
                                    .child(caption("接收演示提醒"))
                                    .child(hint("适合和其他偏好一起阅读、勾选。")),
                            )
                            .on_change(cx.listener(|this, checked, _, cx| {
                                this.enabled = *checked;
                                cx.notify();
                            })),
                    ),
                );
            }
            Variant::Context => {
                body = body.child(
                    Button::new(self.id("toggle"))
                        .accessibility_label("演示提醒")
                        .toggled(self.enabled)
                        .ghost()
                        .w_full()
                        .h(px(100.))
                        .px_4()
                        .border_1()
                        .border_color(rgb(if self.enabled { ACCENT } else { LINE }))
                        .bg(rgb(if self.enabled { SOFT } else { SURFACE }))
                        .rounded_lg()
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
                                        .gap_2()
                                        .child(caption("演示提醒"))
                                        .child(hint(if self.enabled {
                                            "已开启 · 按下即可关闭"
                                        } else {
                                            "已关闭 · 按下即可开启"
                                        })),
                                )
                                .child(
                                    div()
                                        .size(px(32.))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .rounded_full()
                                        .bg(rgb(if self.enabled { ACCENT } else { BG }))
                                        .text_color(rgb(if self.enabled { SURFACE } else { MUTED }))
                                        .child(if self.enabled { "✓" } else { "+" }),
                                ),
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.enabled = !this.enabled;
                            cx.notify();
                        })),
                );
            }
        }
        body.child(hint(setting_description))
            .child(
                div()
                    .mt_3()
                    .p_4()
                    .bg(rgb(BG))
                    .rounded_md()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(div().size(px(6.)).rounded_full().bg(rgb(if self.enabled {
                                ACCENT
                            } else {
                                MUTED
                            })))
                            .child(caption(if self.enabled {
                                "提醒已就绪"
                            } else {
                                "提醒已暂停"
                            })),
                    )
                    .child(hint(if self.enabled {
                        "下一条演示事件可以出现在提醒列表。"
                    } else {
                        "演示事件将保持静默，不弹出提醒。"
                    })),
            )
            .child(self.status("本列状态", if self.enabled { "开启" } else { "关闭" }))
            .into_any_element()
    }

    fn tab_content(&self) -> Div {
        let (eyebrow, title, text) = match self.tab {
            1 => (
                "02 / EVIDENCE",
                "判断的依据",
                "连续四个交易日收盘涨停；此处为固定模拟证据。",
            ),
            2 => (
                "03 / NOTES",
                "人工补充",
                "观察分歧后的承接。备注与证据分开呈现。",
            ),
            _ => (
                "01 / OVERVIEW",
                "研究对象",
                "银力科技 · 002731。先看概览，再进入证据和备注。",
            ),
        };
        div()
            .flex()
            .flex_col()
            .gap_3()
            .min_w_0()
            .child(hint(eyebrow))
            .child(
                div()
                    .text_size(px(18.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(title),
            )
            .child(hint(text))
            .child(div().h(px(1.)).w_full().bg(rgb(LINE)))
            .child(hint("10.14 · 模拟资料"))
    }

    fn render_tabs(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut body = self.body();
        match self.variant {
            Variant::Precision => {
                body = body
                    .child(div().flex().border_b_1().border_color(rgb(LINE)).children(
                        TAB_LABELS.into_iter().enumerate().map(|(index, label)| {
                            Button::new(self.id(&format!("tab-{index}")))
                                .ghost()
                                .flex_1()
                                .h(px(40.))
                                .rounded_none()
                                .text_color(rgb(if self.tab == index { ACCENT } else { MUTED }))
                                .border_b_2()
                                .border_color(rgb(if self.tab == index { ACCENT } else { LINE }))
                                .label(label)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.tab = index;
                                    cx.notify();
                                }))
                        }),
                    ))
                    .child(div().py_4().child(self.tab_content()));
            }
            Variant::Calm => {
                body = body
                    .child(
                        div()
                            .flex()
                            .gap_1()
                            .p_1()
                            .border_1()
                            .border_color(rgb(LINE))
                            .rounded_lg()
                            .children(TAB_LABELS.into_iter().enumerate().map(|(index, label)| {
                                action(self.id(&format!("tab-{index}")), self.variant, label)
                                    .flex_1()
                                    .h(px(38.))
                                    .selected(self.tab == index)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.tab = index;
                                        cx.notify();
                                    }))
                            })),
                    )
                    .child(surface(self.variant).p_4().child(self.tab_content()));
            }
            Variant::Context => {
                body = body.child(
                    div()
                        .flex()
                        .gap_4()
                        .min_h(px(232.))
                        .child(
                            div()
                                .w(px(84.))
                                .flex_shrink_0()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .children(TAB_LABELS.into_iter().enumerate().map(
                                    |(index, label)| {
                                        Button::new(self.id(&format!("tab-{index}")))
                                            .ghost()
                                            .w_full()
                                            .h(px(37.))
                                            .selected(self.tab == index)
                                            .child(
                                                div()
                                                    .w_full()
                                                    .flex()
                                                    .items_center()
                                                    .justify_between()
                                                    .gap_1()
                                                    .child(label)
                                                    .child(hint(["1", "4", "1"][index])),
                                            )
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.tab = index;
                                                cx.notify();
                                            }))
                                    },
                                )),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .pl_4()
                                .py_2()
                                .border_l_1()
                                .border_color(rgb(LINE))
                                .child(self.tab_content()),
                        ),
                );
            }
        }
        body.child(self.status("当前内容", TAB_LABELS[self.tab]))
            .into_any_element()
    }

    fn filter_preview(&self) -> Div {
        let records = [
            ("银力科技", [true, false, true]),
            ("星河设备", [true, false, false]),
            ("远景材料", [false, true, true]),
        ];
        let matches: Vec<_> = records
            .into_iter()
            .filter(|(_, flags)| {
                self.filters
                    .iter()
                    .zip(flags.iter())
                    .all(|(active, flag)| !active || *flag)
            })
            .collect();
        let mut preview = div().flex().flex_col().gap_2().child(hint(format!(
            "同时满足已选条件 · {} 条模拟记录",
            matches.len()
        )));
        for (name, flags) in &matches {
            preview = preview.child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .py_2()
                    .border_b_1()
                    .border_color(rgb(LINE))
                    .child(div().text_size(px(13.)).child(*name))
                    .child(hint(if flags[0] { "已确认" } else { "待观察" })),
            );
        }
        if matches.is_empty() {
            preview = preview.child(
                div()
                    .p_4()
                    .bg(rgb(BG))
                    .rounded_md()
                    .child(hint("没有记录同时满足这些条件。取消一个条件即可扩大范围。")),
            );
        }
        preview
    }

    fn render_tokens(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut body = self.body().child(caption("状态与筛选"));
        match self.variant {
            Variant::Precision => {
                body = body
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(div().size(px(6.)).rounded_full().bg(rgb(ACCENT)))
                            .child(div().text_size(px(13.)).child("已确认"))
                            .child(hint("只读状态")),
                    )
                    .child(hint("筛选条件 · 点击切换"))
                    .child(div().flex().flex_wrap().gap_2().children(
                        FILTERS.into_iter().enumerate().map(|(index, label)| {
                            action(self.id(&format!("filter-{index}")), self.variant, label)
                                .small()
                                .selected(self.filters[index])
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.filters[index] = !this.filters[index];
                                    cx.notify();
                                }))
                        }),
                    ));
            }
            Variant::Calm => {
                body = body
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .px_2()
                                    .py_1()
                                    .border_1()
                                    .border_color(rgb(LINE))
                                    .rounded_md()
                                    .text_size(px(12.))
                                    .child("已确认"),
                            )
                            .child(hint("只读状态")),
                    )
                    .child(hint("筛选条件 · 描边选中"))
                    .child(div().flex().flex_wrap().gap_2().children(
                        FILTERS.into_iter().enumerate().map(|(index, label)| {
                            Button::new(self.id(&format!("filter-{index}")))
                                .ghost()
                                .h(px(33.))
                                .px_3()
                                .border_1()
                                .border_color(rgb(if self.filters[index] { ACCENT } else { LINE }))
                                .bg(rgb(if self.filters[index] { SOFT } else { SURFACE }))
                                .rounded_full()
                                .label(format!(
                                    "{}{}",
                                    if self.filters[index] { "✓ " } else { "+ " },
                                    label
                                ))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.filters[index] = !this.filters[index];
                                    cx.notify();
                                }))
                        }),
                    ));
            }
            Variant::Context => {
                body = body
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(caption("已确认"))
                            .child(hint("只读状态")),
                    )
                    .child(hint("已应用条件 · 点击移除"))
                    .child(
                        div().flex().flex_wrap().gap_2().children(
                            FILTERS
                                .into_iter()
                                .enumerate()
                                .filter(|(index, _)| self.filters[*index])
                                .map(|(index, label)| {
                                    action(
                                        self.id(&format!("filter-{index}")),
                                        self.variant,
                                        format!("{label}  ×"),
                                    )
                                    .small()
                                    .on_click(cx.listener(
                                        move |this, _, _, cx| {
                                            this.filters[index] = false;
                                            cx.notify();
                                        },
                                    ))
                                }),
                        ),
                    )
                    .child(
                        div().flex().flex_wrap().gap_2().children(
                            FILTERS
                                .into_iter()
                                .enumerate()
                                .filter(|(index, _)| !self.filters[*index])
                                .map(|(index, label)| {
                                    Button::new(self.id(&format!("filter-add-{index}")))
                                        .ghost()
                                        .small()
                                        .label(format!("+ {label}"))
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.filters[index] = true;
                                            cx.notify();
                                        }))
                                }),
                        ),
                    );
            }
        }
        body.child(self.filter_preview())
            .child(self.status(
                "已选条件",
                format!(
                    "{} 项",
                    self.filters.iter().filter(|selected| **selected).count()
                ),
            ))
            .into_any_element()
    }
}

impl Render for SelectionSamples {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content = match self.family {
            Family::Choice => self.render_choice(cx),
            Family::Toggle => self.render_toggle(cx),
            Family::Tabs => self.render_tabs(cx),
            Family::Tokens => self.render_tokens(cx),
            _ => self
                .body()
                .child(hint("选择一个基础组件查看样例。"))
                .into_any_element(),
        };
        div()
            .id(self.id("selection-sample"))
            .w_full()
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                if event.keystroke.key == "escape" && this.choice_open {
                    this.choice_open = false;
                    cx.stop_propagation();
                    cx.notify();
                }
            }))
            .child(content)
    }
}
