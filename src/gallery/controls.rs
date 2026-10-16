//! Interactive control specimens. Values and receipts are local to the gallery.
use crate::gallery::foundation::{
    ACCENT, BG, Family, INK, LINE, SOFT, UP, Variant, action, caption, hint, surface,
};
use gpui_kit::{
    component::{
        Disableable, Selectable, Sizable,
        button::ButtonVariants,
        input::{Input, InputEvent, InputState},
    },
    prelude::*,
    *,
};
use tydaro_ui::{
    feedback::{self, FeedbackTone},
    field,
    tokens::COOL_LIGHT,
};

pub struct ControlSamples {
    variant: Variant,
    family: Family,
    focus: FocusHandle,
    name: Entity<InputState>,
    locked_name: Entity<InputState>,
    number: Entity<InputState>,
    saved_name: String,
    editing: bool,
    input_error: Option<String>,
    input_receipt: bool,
    threshold: f64,
    number_error: Option<String>,
    number_receipt: bool,
    exact_number: bool,
    executed: bool,
    executions: usize,
    _subscriptions: Vec<Subscription>,
}

fn validate_name(value: &str) -> Result<String, &'static str> {
    let value = value.trim();
    if value.is_empty() {
        Err("请填写观察名称")
    } else if value.chars().count() > 12 {
        Err("名称最多 12 个字")
    } else {
        Ok(value.to_string())
    }
}

fn parse_threshold(value: &str) -> Result<f64, &'static str> {
    use tydaro_ui::field::{NumberError, NumberRange};
    NumberRange {
        min: 0.5,
        max: 10.,
        decimals: 1,
    }
    .parse(value)
    .map_err(|error| match error {
        NumberError::Invalid => "请输入有效数字",
        NumberError::OutsideRange => "范围为 0.5% 至 10.0%",
        NumberError::Precision => "请保留最多一位小数",
    })
}

impl ControlSamples {
    pub fn new(variant: Variant, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        let name = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value("重点观察")
                .placeholder("输入观察名称")
        });
        let locked_name = cx.new(|cx| InputState::new(window, cx).default_value("系统默认分组"));
        let number = cx.new(|cx| InputState::new(window, cx).default_value("2.0"));
        let subscriptions = vec![
            cx.subscribe_in(&name, window, |this, _, event, window, cx| match event {
                InputEvent::Change => {
                    this.input_error = None;
                    this.input_receipt = false;
                    cx.notify();
                }
                InputEvent::PressEnter { .. } => this.save_name(window, cx),
                _ => {}
            }),
            cx.subscribe_in(&number, window, |this, _, event, window, cx| match event {
                InputEvent::Change => {
                    this.number_error = None;
                    // Programmatic step/preset changes emit the same event as typing.
                    // Preserve their receipt when the editor matches the committed value.
                    this.number_receipt = parse_threshold(this.number.read(cx).value().as_ref())
                        .is_ok_and(|value| (value - this.threshold).abs() < 0.000_001);
                    cx.notify();
                }
                InputEvent::PressEnter { .. } => this.save_number(window, cx),
                _ => {}
            }),
        ];
        Self {
            variant,
            family: Family::Actions,
            focus,
            name,
            locked_name,
            number,
            saved_name: "重点观察".to_string(),
            editing: false,
            input_error: None,
            input_receipt: false,
            threshold: 2.,
            number_error: None,
            number_receipt: false,
            exact_number: false,
            executed: false,
            executions: 0,
            _subscriptions: subscriptions,
        }
    }

    pub fn set_family(&mut self, family: Family, cx: &mut Context<Self>) {
        self.family = family;
        cx.notify();
    }

    #[cfg(feature = "test-support")]
    pub fn snapshot(&self, _: &App) -> (String, f64, bool, bool) {
        (
            self.saved_name.clone(),
            self.threshold,
            self.executed,
            self.editing,
        )
    }

    /// Container, name editor and numeric editor focus, read from real handles.
    #[cfg(feature = "test-support")]
    pub fn focus_snapshot(&self, window: &Window, cx: &App) -> (bool, bool, bool) {
        (
            self.focus.is_focused(window),
            self.name.read(cx).focus_handle(cx).is_focused(window),
            self.number.read(cx).focus_handle(cx).is_focused(window),
        )
    }

    fn id(&self, suffix: &str) -> String {
        format!("gallery-{}-{suffix}", self.variant.key())
    }

    fn save_name(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match validate_name(self.name.read(cx).value().as_ref()) {
            Ok(name) => {
                self.saved_name = name;
                self.input_error = None;
                self.input_receipt = true;
                if self.variant == Variant::Context {
                    // Button mouse-down preserves the editor's focus. Transfer it
                    // before unmounting the editor so hidden input cannot consume keys.
                    window.focus(&self.focus, cx);
                }
                self.editing = false;
            }
            Err(error) => {
                self.input_error = Some(error.to_string());
                self.input_receipt = false;
            }
        }
        cx.notify();
    }

    fn edit_name(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.editing = true;
        self.input_receipt = false;
        self.input_error = None;
        let value = self.saved_name.clone();
        self.name.update(cx, |input, cx| {
            input.set_value(value, window, cx);
            input.focus(window, cx);
            input.select_all(window, cx);
        });
        cx.notify();
    }

    fn save_number(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match parse_threshold(self.number.read(cx).value().as_ref()) {
            Ok(value) => {
                self.threshold = value;
                self.number_error = None;
                self.number_receipt = true;
                if self.variant == Variant::Context {
                    window.focus(&self.focus, cx);
                }
                self.exact_number = false;
            }
            Err(error) => {
                self.number_error = Some(error.to_string());
                self.number_receipt = false;
            }
        }
        cx.notify();
    }

    fn set_number(&mut self, value: f64, window: &mut Window, cx: &mut Context<Self>) {
        self.threshold = value.clamp(0.5, 10.);
        let text = format!("{:.1}", self.threshold);
        self.number
            .update(cx, |input, cx| input.set_value(text, window, cx));
        self.number_error = None;
        self.number_receipt = true;
        if self.variant == Variant::Context && self.exact_number {
            window.focus(&self.focus, cx);
        }
        self.exact_number = false;
        cx.notify();
    }

    fn name_input(&self) -> Input {
        field::validated_input(
            &self.name,
            self.id("name-input"),
            self.input_error.is_some(),
            COOL_LIGHT,
        )
        .aria_label("观察名称")
    }

    fn input_status(&self) -> Div {
        let (text, tone) = if let Some(error) = &self.input_error {
            (error.clone(), FeedbackTone::Error)
        } else if self.input_receipt {
            (
                format!("已保存 · {}", self.saved_name),
                FeedbackTone::Success,
            )
        } else {
            ("1–12 个字 · Enter 保存".to_string(), FeedbackTone::Neutral)
        };
        feedback::inline(text, tone, COOL_LIGHT)
    }

    fn number_status(&self) -> Div {
        let (text, tone) = if let Some(error) = &self.number_error {
            (error.clone(), FeedbackTone::Error)
        } else if self.number_receipt {
            (
                format!("已应用阈值 {:.1}%", self.threshold),
                FeedbackTone::Success,
            )
        } else {
            (
                "允许 0.5%–10.0%，最多一位小数".to_string(),
                FeedbackTone::Neutral,
            )
        };
        feedback::inline(text, tone, COOL_LIGHT)
    }

    fn actions(&self, cx: &mut Context<Self>) -> Div {
        let variant = self.variant;
        let run = |id: String, label: &'static str| {
            action(id, variant, label).on_click(cx.listener(|this, _, _, cx| {
                this.executed = true;
                this.executions += 1;
                cx.notify();
            }))
        };
        let mut body = surface(variant)
            .flex()
            .flex_col()
            .p_4()
            .gap_4()
            .child(caption("重点观察"));
        body = match variant {
            Variant::Precision => body
                .child(hint("动作集中在工具栏，适合连续处理。"))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_1()
                        .child(run(self.id("action-run"), "加入观察").primary())
                        .child(
                            action(self.id("action-reset"), variant, "重置")
                                .ghost()
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.executed = false;
                                    cx.notify();
                                })),
                        )
                        .child(action(self.id("action-disabled"), variant, "导出").disabled(true)),
                )
                .child(hint("导出不可用：当前没有研究记录。"))
                .child(div().h(px(1.)).bg(rgb(LINE)))
                .child(caption(if self.executed {
                    "观察列表 · 已加入"
                } else {
                    "观察列表 · 未加入"
                }))
                .child(hint(format!(
                    "本列已执行 {} 次 · 仅本地演示",
                    self.executions
                ))),
            Variant::Calm => body
                .child(hint("先突出一个主要动作，再提供次要路径。"))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .mt_3()
                        .child(
                            run(self.id("action-run"), "加入重点观察")
                                .primary()
                                .w_full()
                                .h(px(46.)),
                        )
                        .child(
                            action(self.id("action-reset"), variant, "继续整理")
                                .w_full()
                                .h(px(40.))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.executed = false;
                                    cx.notify();
                                })),
                        ),
                )
                .child(hint(if self.executed {
                    "已加入观察列表，可继续整理。"
                } else {
                    "确认后仅改变此示例的观察状态。"
                }))
                .child(
                    action(self.id("action-disabled"), variant, "导出研究记录")
                        .ghost()
                        .disabled(true),
                )
                .child(hint("没有研究记录时，保留禁用原因。")),
            Variant::Context => {
                let row = div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .p_3()
                    .bg(rgb(SOFT))
                    .rounded(px(8.));
                body.child(hint("就地执行、就地回执，保留撤销入口。"))
                    .child(if self.executed {
                        row.child(
                            div()
                                .flex()
                                .justify_between()
                                .items_center()
                                .child(caption("已加入观察").text_color(rgb(ACCENT)))
                                .child(
                                    action(self.id("action-undo"), variant, "撤销")
                                        .ghost()
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.executed = false;
                                            cx.notify();
                                        })),
                                ),
                        )
                        .child(hint("重点观察 · 此操作可撤销"))
                    } else {
                        row.child(caption("将此对象加入观察列表"))
                            .child(hint("操作结果会留在当前上下文。"))
                            .child(run(self.id("action-run"), "加入观察 →").primary())
                    })
                    .child(hint(if self.executed {
                        "操作后，主要按钮让位于结果和撤销。"
                    } else {
                        "执行不会跳页，也不会关闭当前对象。"
                    }))
                    .child(
                        action(self.id("action-disabled"), variant, "导出记录")
                            .ghost()
                            .disabled(true),
                    )
                    .child(hint("导出不可用：当前没有研究记录。"))
            }
        };
        body
    }

    fn inputs(&self, window: &Window, cx: &mut Context<Self>) -> Div {
        let variant = self.variant;
        let name_focused = self.name.read(cx).focus_handle(cx).is_focused(window);
        let save = action(self.id("name-save"), variant, "保存")
            .primary()
            .on_click(cx.listener(|this, _, window, cx| this.save_name(window, cx)));
        let clear = action(self.id("name-clear"), variant, "清空")
            .ghost()
            .on_click(cx.listener(|this, _, window, cx| {
                this.name.update(cx, |input, cx| {
                    input.set_value("", window, cx);
                    input.focus(window, cx);
                });
                this.input_error = None;
                this.input_receipt = false;
                cx.notify();
            }));
        let mut body = surface(variant).flex().flex_col().p_4().gap_4();
        body = match variant {
            Variant::Precision => body
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(caption("观察名称"))
                        .child(hint(format!(
                            "{}/12",
                            self.name.read(cx).value().chars().count()
                        ))),
                )
                .child(self.name_input())
                .child(self.input_status())
                .child(div().flex().gap_2().child(save).child(clear))
                .child(div().h(px(1.)).bg(rgb(LINE)).mt_3())
                .child(caption("系统分组 · 不可编辑"))
                .child(field::input(&self.locked_name, self.id("name-locked")).disabled(true))
                .child(hint("完整字段边界，校验信息固定在下方。")),
            Variant::Calm => body
                .child(caption("观察设置"))
                .child(hint("标签与内容同行，节省表单纵向空间。"))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_3()
                        .py_3()
                        .border_b_1()
                        .border_color(rgb(if self.input_error.is_some() {
                            UP
                        } else if name_focused {
                            ACCENT
                        } else {
                            LINE
                        }))
                        .child(hint("名称").w(px(38.)).flex_shrink_0())
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .child(self.name_input().small().appearance(false)),
                        ),
                )
                .child(self.input_status())
                .child(div().flex().justify_end().gap_2().child(clear).child(save))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .py_3()
                        .border_b_1()
                        .border_color(rgb(LINE))
                        .child(hint("归属"))
                        .child(hint("系统默认分组 · 只读")),
                )
                .child(hint("只读值回归文字，避免重复的输入框轮廓。")),
            Variant::Context => {
                body = body.child(caption("观察名称"));
                if self.editing {
                    body = body
                        .child(self.name_input())
                        .child(self.input_status())
                        .child(
                            div()
                                .flex()
                                .gap_2()
                                .child(save)
                                .child(
                                    action(self.id("name-cancel"), variant, "取消")
                                        .ghost()
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            window.focus(&this.focus, cx);
                                            let value = this.saved_name.clone();
                                            this.name.update(cx, |input, cx| {
                                                input.set_value(value, window, cx)
                                            });
                                            this.editing = false;
                                            this.input_error = None;
                                            this.input_receipt = false;
                                            cx.notify();
                                        })),
                                )
                                .child(clear),
                        );
                } else {
                    body = body
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .gap_2()
                                .py_3()
                                .child(
                                    div()
                                        .text_size(px(22.))
                                        .text_color(rgb(INK))
                                        .child(self.saved_name.clone()),
                                )
                                .child(
                                    action(self.id("name-edit"), variant, "编辑")
                                        .ghost()
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.edit_name(window, cx)
                                        })),
                                ),
                        )
                        .child(self.input_status());
                }
                body.child(div().h(px(1.)).bg(rgb(LINE)))
                    .child(hint("阅读时只显示值；需要修改时再展开编辑器。"))
                    .child(hint("取消保留原值，保存后回到阅读态。"))
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .pt_3()
                            .child(hint("系统默认分组"))
                            .child(hint("只读")),
                    )
            }
        };
        body
    }

    fn numbers(&self, cx: &mut Context<Self>) -> Div {
        let variant = self.variant;
        let field = || {
            field::validated_input(
                &self.number,
                self.id("number-input"),
                self.number_error.is_some(),
                COOL_LIGHT,
            )
            .aria_label("涨幅阈值，百分比")
            .suffix("%")
        };
        let apply = action(self.id("number-apply"), variant, "应用阈值")
            .primary()
            .on_click(cx.listener(|this, _, window, cx| this.save_number(window, cx)));
        let mut body = surface(variant)
            .flex()
            .flex_col()
            .p_4()
            .gap_4()
            .child(caption("涨幅阈值"))
            .child(hint("重点观察 · 模拟预警条件"));
        body = match variant {
            Variant::Precision => body
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(div().flex_1().min_w_0().child(field()))
                        .child(apply),
                )
                .child(self.number_status())
                .child(
                    div()
                        .bg(rgb(BG))
                        .p_3()
                        .flex()
                        .justify_between()
                        .child(hint("当前生效"))
                        .child(caption(format!("≥ {:.1}%", self.threshold))),
                )
                .child(hint("输入任意精确值，校验通过后再应用。"))
                .child(hint("试试输入 12、文字或两位小数。")),
            Variant::Calm => body
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .py_4()
                        .child(
                            action(self.id("number-minus"), variant, "−")
                                .h(px(42.))
                                .w(px(42.))
                                .disabled(self.threshold <= 0.5)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.set_number(this.threshold - 0.5, window, cx)
                                })),
                        )
                        .child(
                            div()
                                .flex()
                                .items_baseline()
                                .gap_1()
                                .child(
                                    div()
                                        .text_size(px(38.))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(rgb(INK))
                                        .child(format!("{:.1}", self.threshold)),
                                )
                                .child(hint("%")),
                        )
                        .child(
                            action(self.id("number-plus"), variant, "+")
                                .h(px(42.))
                                .w(px(42.))
                                .disabled(self.threshold >= 10.)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.set_number(this.threshold + 0.5, window, cx)
                                })),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .justify_between()
                        .child(hint("0.5%"))
                        .child(hint("每次调整 0.5%"))
                        .child(hint("10.0%")),
                )
                .child(self.number_status())
                .child(hint("逐步微调立即生效；到达边界时禁用对应按钮。")),
            Variant::Context => {
                body = body
                    .child(
                        div()
                            .flex()
                            .items_baseline()
                            .gap_1()
                            .child(
                                div()
                                    .text_size(px(38.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(format!("{:.1}", self.threshold)),
                            )
                            .child(hint("%")),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_1()
                            .children([1., 2., 3., 5.].into_iter().map(|value| {
                                action(
                                    self.id(&format!("number-tick-{}", value as usize)),
                                    variant,
                                    format!("{value:.0}%"),
                                )
                                .flex_1()
                                .selected((self.threshold - value).abs() < 0.001)
                                .on_click(cx.listener(
                                    move |this, _, window, cx| this.set_number(value, window, cx),
                                ))
                            })),
                    )
                    .child(hint("常用刻度 · 点击即可应用"));
                if self.exact_number {
                    body = body.child(
                        div()
                            .flex()
                            .gap_2()
                            .items_center()
                            .child(div().flex_1().min_w_0().child(field()))
                            .child(apply),
                    );
                } else {
                    body = body.child(
                        action(self.id("number-exact"), variant, "输入精确值")
                            .ghost()
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.exact_number = true;
                                let text = format!("{:.1}", this.threshold);
                                this.number.update(cx, |input, cx| {
                                    input.set_value(text, window, cx);
                                    input.focus(window, cx);
                                    input.select_all(window, cx);
                                });
                                cx.notify();
                            })),
                    );
                }
                body.child(self.number_status())
                    .child(hint("常用值优先；精确编辑按需出现。"))
            }
        };
        body
    }
}

impl Render for ControlSamples {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id(self.id("controls"))
            .track_focus(&self.focus)
            .w_full()
            .min_w_0()
            .text_color(rgb(INK))
            .child(match self.family {
                Family::Actions => self.actions(cx),
                Family::Input => self.inputs(window, cx),
                Family::Number => self.numbers(cx),
                _ => surface(self.variant).child(hint("选择动作、文本输入或数值阈值组件。")),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_threshold, validate_name};

    #[test]
    fn names_validate_trimmed_unicode_characters() {
        assert_eq!(validate_name(" 重点观察 "), Ok("重点观察".to_string()));
        assert!(validate_name("  ").is_err());
        assert!(validate_name(&"观".repeat(13)).is_err());
    }

    #[test]
    fn thresholds_reject_non_finite_out_of_range_and_excess_precision() {
        assert_eq!(parse_threshold("2.0"), Ok(2.));
        assert_eq!(parse_threshold("0.5"), Ok(0.5));
        assert_eq!(parse_threshold("10"), Ok(10.));
        for value in ["NaN", "inf", "0.4", "10.1", "2.25", "文字"] {
            assert!(parse_threshold(value).is_err(), "accepted {value}");
        }
    }
}
