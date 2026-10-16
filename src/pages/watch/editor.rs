use super::*;

impl WatchPage {
    pub(super) fn report_pending(cx: &mut Context<Self>, error: Option<String>) {
        let key = format!("watch-editor-{:?}", cx.entity_id());
        desktop_state::report_pending_input(cx, key, error);
    }

    pub(super) fn refresh_pending_input(&self, cx: &mut Context<Self>) {
        let state = cx.global::<DesktopState>();
        let group_changed = self.editor.as_ref().is_some_and(|editor| {
            let name = self.name.read(cx).value();
            let note = self.note.read(cx).value();
            if let Some(id) = &editor.id {
                state
                    .watch_groups
                    .iter()
                    .find(|g| &g.id == id)
                    .is_none_or(|original| {
                        name.as_str() != original.name
                            || note.as_str() != original.note
                            || editor.symbols != original.symbols
                            || editor.color != original.color
                    })
            } else {
                !name.is_empty()
                    || !note.is_empty()
                    || !editor.symbols.is_empty()
                    || editor.color != GROUP_COLORS[0]
            }
        });
        let rules_changed = self.rules_editor.as_ref().is_some_and(|rules| {
            rules.len() != state.rules.len()
                || rules.iter().zip(&self.rule_inputs).any(|(rule, input)| {
                    state
                        .rules
                        .iter()
                        .find(|r| r.id == rule.id)
                        .is_none_or(|original| {
                            rule.enabled != original.enabled
                                || input.read(cx).value().as_str()
                                    != format!("{:.2}", original.threshold)
                        })
                })
        });
        Self::report_pending(
            cx,
            (group_changed || rules_changed).then(|| "盯盘编辑尚未保存，请保存或取消。".into()),
        );
    }

    pub(super) fn cancel_editor(&mut self, cx: &mut Context<Self>) {
        self.editor = None;
        self.rules_editor = None;
        self.rule_subscriptions.clear();
        self.error = None;
        Self::report_pending(cx, None);
        cx.notify();
    }

    pub(super) fn begin_group(
        &mut self,
        group: Option<WatchGroup>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_editing() {
            cx.emit(DesktopEvent::Notice("请先保存或取消当前盯盘编辑".into()));
            return;
        }
        let (name, note, editor) = if let Some(group) = group {
            (
                group.name,
                group.note,
                GroupEditor {
                    id: Some(group.id),
                    symbols: group.symbols,
                    color: group.color,
                    deleting: false,
                },
            )
        } else {
            (
                String::new(),
                String::new(),
                GroupEditor {
                    id: None,
                    symbols: Vec::new(),
                    color: GROUP_COLORS[0],
                    deleting: false,
                },
            )
        };
        self.editor = Some(editor);
        self.rules_editor = None;
        self.error = None;
        self.name.update(cx, |input, cx| {
            input.set_value(name, window, cx);
            input.focus(window, cx);
            input.select_all(window, cx);
        });
        self.note
            .update(cx, |input, cx| input.set_value(note, window, cx));
        self.search
            .update(cx, |input, cx| input.set_value("", window, cx));
        cx.notify();
    }

    pub(super) fn save_group(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = &self.editor else { return };
        let name = self.name.read(cx).value().trim().to_owned();
        let note = self.note.read(cx).value().trim().to_owned();
        let invalid =
            if name.is_empty() || name.chars().count() > 24 || name.chars().any(char::is_control) {
                Some("分组名称需为 1–24 个字符，不能包含控制字符。")
            } else if cx
                .global::<DesktopState>()
                .watch_groups
                .iter()
                .any(|group| Some(&group.id) != editor.id.as_ref() && group.name == name)
            {
                Some("已有同名分组，请使用不同的名称。")
            } else if editor.id.is_none() && cx.global::<DesktopState>().watch_groups.len() >= 128 {
                Some("最多支持 128 个盯盘分组，请先整理已有分组。")
            } else if note.chars().count() > 160 {
                Some("观察备注最多 160 个字符。")
            } else {
                None
            };
        if let Some(error) = invalid {
            self.error = Some(error.into());
            Self::report_pending(cx, Some(error.into()));
            cx.notify();
            return;
        }
        let prior_id = editor.id.clone();
        let symbols = editor.symbols.clone();
        let color = editor.color;
        let count = symbols.len();
        let saved_id = match desktop_state::dispatch(
            cx,
            Command::SaveWatchGroup {
                id: prior_id,
                name: name.clone(),
                note,
                color,
                symbols,
            },
        )
        .and_then(CommandResult::into_id)
        {
            Ok(id) => id,
            Err(error) => {
                self.error = Some(error.to_string());
                Self::report_pending(cx, self.error.clone());
                cx.notify();
                return;
            }
        };
        let index = cx
            .global::<DesktopState>()
            .watch_groups
            .iter()
            .position(|group| group.id == saved_id)
            .unwrap_or(0);
        self.selected_group = Some(saved_id);
        self.scroll.scroll_to_item(1 + index / self.actual_columns);
        self.editor = None;
        self.error = None;
        Self::report_pending(cx, None);
        cx.emit(DesktopEvent::Notice(format!(
            "已保存「{name}」，共 {count} 只股票"
        )));
        cx.notify();
    }

    pub(super) fn delete_group(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.editor.as_ref().and_then(|editor| editor.id.clone()) else {
            return;
        };
        if let Err(error) = desktop_state::dispatch(cx, Command::DeleteWatchGroup(id.clone())) {
            self.error = Some(error.to_string());
            cx.notify();
            return;
        }
        if self.selected_group.as_ref() == Some(&id) {
            self.selected_group = None;
            self.selected_symbol = None;
        }
        self.editor = None;
        self.error = None;
        Self::report_pending(cx, None);
        cx.emit(DesktopEvent::Notice(
            "已删除盯盘分组，历史异动记录仍保留".into(),
        ));
        cx.notify();
    }

    pub(super) fn begin_rules(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_editing() {
            cx.emit(DesktopEvent::Notice("请先保存或取消当前盯盘编辑".into()));
            return;
        }
        let rules = cx.global::<DesktopState>().rules.clone();
        self.rule_inputs = rules
            .iter()
            .map(|rule| {
                cx.new(|cx| {
                    InputState::new(window, cx).default_value(format!("{:.2}", rule.threshold))
                })
            })
            .collect();
        self.rule_subscriptions = self
            .rule_inputs
            .iter()
            .map(|input| {
                cx.subscribe(input, |this, _, event: &InputEvent, cx| {
                    if matches!(event, InputEvent::Change) {
                        this.refresh_pending_input(cx);
                        cx.notify();
                    }
                })
            })
            .collect();
        self.rules_editor = Some(rules);
        self.editor = None;
        self.error = None;
        if let Some(input) = self.rule_inputs.first() {
            input.update(cx, |input, cx| input.focus(window, cx));
        }
        cx.notify();
    }

    pub(super) fn save_rules(&mut self, cx: &mut Context<Self>) {
        let Some(mut rules) = self.rules_editor.clone() else {
            return;
        };
        for (rule, input) in rules.iter_mut().zip(&self.rule_inputs) {
            match input.read(cx).value().trim().parse::<f64>() {
                Ok(value) if value.is_finite() && value > 0. && value <= 100. => {
                    rule.threshold = value
                }
                _ => {
                    self.error = Some("规则阈值请输入大于 0、且不超过 100 的数值。".into());
                    Self::report_pending(cx, self.error.clone());
                    cx.notify();
                    return;
                }
            }
        }
        if let Err(error) = desktop_state::dispatch(cx, Command::ReplaceRules(rules)) {
            self.error = Some(error.to_string());
            Self::report_pending(cx, self.error.clone());
            cx.notify();
            return;
        }
        self.rules_editor = None;
        self.rule_subscriptions.clear();
        self.error = None;
        Self::report_pending(cx, None);
        cx.emit(DesktopEvent::Notice(
            "已保存演示规则；点击“演示异动”可验证新阈值".into(),
        ));
        cx.notify();
    }

    pub(super) fn group_editor(&self, cx: &mut Context<Self>) -> AnyElement {
        let editor = self.editor.as_ref().expect("group editor");
        let query = self.search.read(cx).value().trim().to_lowercase();
        let mut members = div()
            .id("watch-member-list")
            .max_h(px(245.))
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .border_1()
            .border_color(rgb(LINE))
            .rounded(px(5.));
        let mut matches = 0;
        for symbol in Symbol::A_SHARES.into_iter().chain(Symbol::LEGACY_DEFAULTS) {
            if !query.is_empty()
                && !symbol.code().to_lowercase().contains(&query)
                && !symbol.name().contains(&query)
            {
                continue;
            }
            matches += 1;
            let checked = editor.symbols.contains(&symbol);
            members = members.child(
                div()
                    .id(SharedString::from(format!(
                        "watch-member-row-{}",
                        symbol.code()
                    )))
                    .test_support()
                    .h(px(38.))
                    .flex_shrink_0()
                    .px_3()
                    .flex()
                    .items_center()
                    .gap_3()
                    .cursor_pointer()
                    .border_b_1()
                    .border_color(rgb(0xf0f3f5))
                    .hover(|style| style.bg(rgb(0xf0f8f7)))
                    .child(
                        Checkbox::new(SharedString::from(format!(
                            "watch-member-{}",
                            symbol.code()
                        )))
                        .checked(checked)
                        .accessibility_label(format!("选择 {} {}", symbol.name(), symbol.code()))
                        .on_change(cx.listener(
                            move |this, checked, _, cx| {
                                cx.stop_propagation();
                                if let Some(editor) = &mut this.editor {
                                    editor.symbols.retain(|item| *item != symbol);
                                    if *checked {
                                        editor.symbols.push(symbol);
                                    }
                                }
                                this.refresh_pending_input(cx);
                                cx.notify();
                            },
                        )),
                    )
                    .child(div().w(px(86.)).child(symbol.name()))
                    .child(small(symbol.code()))
                    .child(div().flex_1())
                    .child(tag(symbol.market(), MUTED))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(editor) = &mut this.editor {
                            if editor.symbols.contains(&symbol) {
                                editor.symbols.retain(|item| *item != symbol);
                            } else {
                                editor.symbols.push(symbol);
                            }
                        }
                        this.refresh_pending_input(cx);
                        cx.notify();
                    })),
            );
        }
        if matches == 0 {
            members = members.child(
                div()
                    .py_6()
                    .text_center()
                    .child(small("模拟目录中没有匹配股票")),
            );
        }
        let mut colors = div().flex().items_center().gap_3();
        for color in GROUP_COLORS {
            colors = colors.child(
                div()
                    .id(SharedString::from(format!("watch-color-{color}")))
                    .size(px(23.))
                    .rounded_full()
                    .bg(rgb(color))
                    .border_2()
                    .border_color(rgb(if color == editor.color { INK } else { 0xffffff }))
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(editor) = &mut this.editor {
                            editor.color = color;
                        }
                        this.refresh_pending_input(cx);
                        cx.notify();
                    })),
            );
        }
        let mut content = div()
            .flex()
            .flex_col()
            .p_5()
            .gap_4()
            .child(dialog_title(
                if editor.id.is_some() {
                    "管理盯盘分组"
                } else {
                    "新建盯盘分组"
                },
                "每个分组独立管理股票，不会修改行情组件中的股票列表。",
                "watch-group-close",
                cx.listener(|this, _, _, cx| {
                    this.cancel_editor(cx);
                }),
            ))
            .child(field(
                "分组名称",
                Input::new(&self.name).id("watch-group-name").w_full(),
            ))
            .child(field(
                "观察备注",
                Input::new(&self.note).id("watch-group-note").w_full(),
            ))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_4()
                    .child(small("识别颜色"))
                    .child(colors),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(bold("选择成员"))
                    .child(small(format!("已选择 {} 只", editor.symbols.len()))),
            )
            .child(Input::new(&self.search).id("watch-group-search").w_full())
            .child(members);
        if let Some(error) = &self.error {
            content = content.child(
                div()
                    .text_size(px(12.))
                    .text_color(rgb(UP))
                    .child(error.clone()),
            );
        }
        if editor.deleting {
            content = content.child(
                div()
                    .p_3()
                    .bg(rgb(0xfff4f4))
                    .border_1()
                    .border_color(rgb(0xf2d6dd))
                    .rounded(px(5.))
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .text_color(rgb(UP))
                            .child("删除这个分组及其成员设置？历史异动记录会保留。"),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(
                                button("watch-delete-confirm", "确认删除分组")
                                    .text_color(rgb(UP))
                                    .on_click(cx.listener(|this, _, _, cx| this.delete_group(cx))),
                            )
                            .child(button("watch-delete-cancel", "保留分组").on_click(
                                cx.listener(|this, _, _, cx| {
                                    if let Some(editor) = &mut this.editor {
                                        editor.deleting = false;
                                    }
                                    this.refresh_pending_input(cx);
                                    cx.notify();
                                }),
                            )),
                    ),
            );
        } else {
            content = content.child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .when(editor.id.is_some(), |row| {
                        row.child(
                            button("watch-group-delete", "删除分组")
                                .ghost()
                                .text_color(rgb(UP))
                                .icon(IconName::Trash)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    if let Some(editor) = &mut this.editor {
                                        editor.deleting = true;
                                    }
                                    this.refresh_pending_input(cx);
                                    cx.notify();
                                })),
                        )
                    })
                    .child(div().flex_1())
                    .child(button("watch-group-cancel", "取消").on_click(cx.listener(
                        |this, _, _, cx| {
                            this.cancel_editor(cx);
                        },
                    )))
                    .child(
                        button("watch-group-save", "保存分组")
                            .primary()
                            .on_click(cx.listener(|this, _, _, cx| this.save_group(cx))),
                    ),
            );
        }
        overlay(
            panel()
                .id("watch-group-dialog")
                .w(px(570.))
                .max_h(relative(0.94))
                .overflow_y_scroll()
                .child(content),
        )
        .into_any_element()
    }

    pub(super) fn rules_editor(&self, cx: &mut Context<Self>) -> AnyElement {
        let rules = self.rules_editor.as_ref().expect("rules editor");
        let mut content = div().p_5().flex().flex_col().gap_5().child(dialog_title(
            "预警规则",
            "本地演示规则，仅在点击“演示异动”时生成事件。",
            "watch-rules-close",
            cx.listener(|this, _, _, cx| {
                this.cancel_editor(cx);
            }),
        ));
        for (index, rule) in rules.iter().enumerate() {
            content = content.child(
                div()
                    .border_1()
                    .border_color(rgb(LINE))
                    .rounded(px(6.))
                    .p_4()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(bold(rule.title.clone()))
                            .child(
                                button(
                                    SharedString::from(format!("watch-rule-enable-{}", rule.id)),
                                    if rule.enabled {
                                        "已启用"
                                    } else {
                                        "已停用"
                                    },
                                )
                                .ghost()
                                .icon(if rule.enabled {
                                    IconName::Check
                                } else {
                                    IconName::Minus
                                })
                                .text_color(rgb(if rule.enabled { TEAL } else { MUTED }))
                                .on_click(cx.listener(
                                    move |this, _, _, cx| {
                                        if let Some(rules) = &mut this.rules_editor {
                                            rules[index].enabled = !rules[index].enabled;
                                        }
                                        this.refresh_pending_input(cx);
                                        cx.notify();
                                    },
                                )),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(
                                div()
                                    .flex_1()
                                    .text_size(px(12.))
                                    .text_color(rgb(MUTED))
                                    .child(rule.label.clone()),
                            )
                            .child(
                                Input::new(&self.rule_inputs[index])
                                    .id(SharedString::from(format!("watch-rule-{}", rule.id)))
                                    .w(px(86.)),
                            )
                            .child(small(rule.unit.clone())),
                    ),
            );
        }
        if let Some(error) = &self.error {
            content = content.child(
                div()
                    .text_size(px(12.))
                    .text_color(rgb(UP))
                    .child(error.clone()),
            );
        }
        content = content
            .child(small(
                "规则状态与阈值随工作区保存。暂停监控不会清空已有异动。",
            ))
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap_2()
                    .child(button("watch-rules-cancel", "取消").on_click(cx.listener(
                        |this, _, _, cx| {
                            this.cancel_editor(cx);
                        },
                    )))
                    .child(
                        button("watch-rules-save", "保存规则")
                            .primary()
                            .on_click(cx.listener(|this, _, _, cx| this.save_rules(cx))),
                    ),
            );
        overlay(
            panel()
                .id("watch-rules-dialog")
                .w(px(530.))
                .max_h(relative(0.94))
                .overflow_y_scroll()
                .child(content),
        )
        .into_any_element()
    }
}
