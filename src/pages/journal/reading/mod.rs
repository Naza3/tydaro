//! Every reading window creates its own input, subscription and focus handle.
use super::*;

impl JournalPage {
    pub(super) fn open_reading(&mut self, source: &Window, cx: &mut Context<Self>) {
        let date = self.date.clone();
        let display = source.display(cx);
        let mut bounds = Bounds::new(
            source.bounds().origin + point(px(40.), px(40.)),
            size(px(870.), px(760.)),
        );
        if let Some(display) = &display {
            let visible = display.visible_bounds();
            bounds.size.width = bounds.size.width.min(visible.size.width);
            bounds.size.height = bounds.size.height.min(visible.size.height);
            bounds.origin.x = bounds
                .origin
                .x
                .max(visible.origin.x)
                .min(visible.right() - bounds.size.width);
            bounds.origin.y = bounds
                .origin
                .y
                .max(visible.origin.y)
                .min(visible.bottom() - bounds.size.height);
        }
        let result = gpui_kit::open_window(
            WindowOptions {
                display_id: display.map(|display| display.id()),
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some(format!("{date} 复盘阅读 · Tydaro").into()),
                    ..Default::default()
                }),
                window_min_size: Some(size(px(600.), px(420.))),
                ..Default::default()
            },
            cx,
            move |window, cx| cx.new(|cx| JournalReading::new(date, window, cx)),
        );
        if let Err(error) = result {
            self.error = Some(format!("无法打开阅读窗口：{error}"));
            cx.notify();
        }
    }
}

struct JournalReading {
    date: String,
    focus: FocusHandle,
    note: Entity<TextareaState>,
    note_synced: String,
    note_error: Option<String>,
    close_confirmation_open: bool,
    _subscription: Subscription,
    _release: Subscription,
}
impl JournalReading {
    fn new(date: String, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let text = cx
            .global::<DesktopState>()
            .journal
            .days
            .get(&date)
            .map(|day| day.note.clone())
            .unwrap_or_default();
        let note = area(text.clone(), "补充当前日期的观察笔记", window, cx);
        let subscription = cx.subscribe(&note, move |this, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                this.persist_note(cx);
                cx.notify();
            }
        });
        let view = cx.weak_entity();
        window.on_window_should_close(cx, move |window, cx| {
            view.update(cx, |this, cx| this.request_close(window, cx))
                .unwrap_or(true)
        });
        let key = format!("journal-note:{:?}", cx.entity_id());
        let release = cx.on_release(move |_, cx| {
            if cx.has_global::<DesktopState>() {
                desktop_state::report_pending_input(cx, key.clone(), None);
            }
        });
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        Self {
            date,
            focus,
            note,
            note_synced: text,
            note_error: None,
            close_confirmation_open: false,
            _subscription: subscription,
            _release: release,
        }
    }
    fn persist_note(&mut self, cx: &mut Context<Self>) -> bool {
        let key = format!("journal-note:{:?}", cx.entity_id());
        self.note_error = publish_note(&self.note, &mut self.note_synced, &self.date, key, cx)
            .err()
            .map(|error| format!("笔记尚未保存：{error}"));
        self.note_error.is_none()
    }

    fn request_close(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        // Native close events and the page button can arrive again while the
        // confirmation is open. Keep one decision for this window's input.
        if self.close_confirmation_open {
            return false;
        }
        if self.persist_note(cx) {
            return true;
        }
        self.close_confirmation_open = true;
        let detail = self.note_error.clone().unwrap_or_default();
        let key = format!("journal-note:{:?}", cx.entity_id());
        let view = cx.weak_entity();
        window.open_dialog(cx, move |dialog, _, _| {
            let key = key.clone();
            let dismissed = view.clone();
            let keep = view.clone();
            let discard = view.clone();
            dialog.title("笔记修改尚未保存").width(px(460.))
                .on_close(move |_, _, cx| {
                    let _ = dismissed.update(cx, |this, _| this.close_confirmation_open = false);
                })
                .child(
                div().flex().flex_col().gap_4().child(detail.clone())
                    .child("继续编辑可保留当前输入。放弃后只丢弃本窗口尚未保存的输入，已保存的笔记与预案保持不变。")
                    .child(div().flex().justify_end().gap_2()
                        .child(action("journal-reading-keep", "继续编辑").primary()
                            .on_click(move |_, window, cx| {
                                let _ = keep.update(cx, |this, _| this.close_confirmation_open = false);
                                window.close_dialog(cx);
                            }))
                        .child(action("journal-reading-discard", "放弃输入并关闭")
                            .on_click(move |_, window, cx| {
                                let _ = discard.update(cx, |this, _| this.close_confirmation_open = false);
                                desktop_state::report_pending_input(cx, key.clone(), None);
                                window.close_dialog(cx);
                                window.remove_window();
                            })))
            )
        });
        false
    }
}
impl Render for JournalReading {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let day = cx
            .global::<DesktopState>()
            .journal
            .days
            .get(&self.date)
            .cloned()
            .unwrap_or_default();
        sync_shared_note(&self.note, &mut self.note_synced, &day.note, window, cx);
        div()
            .id("journal-reading")
            .test_support()
            .track_focus(&self.focus)
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(BG))
            .text_color(rgb(INK))
            .text_size(px(14.))
            .child(
                div()
                    .px_5()
                    .py_4()
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .gap_3()
                    .bg(rgb(0xffffff))
                    .border_b_1()
                    .border_color(rgb(LINE))
                    .child(
                        div()
                            .flex_1()
                            .child(
                                div()
                                    .text_size(px(21.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(format!("{} · 复盘阅读", self.date)),
                            )
                            .child(ui::muted("日期固定 · 内容与主窗口同步 · 关窗保留全部记录")),
                    )
                    .child(
                        action("journal-reading-close", "关闭窗口").on_click(cx.listener(
                            |this, _, window, cx| {
                                if this.request_close(window, cx) {
                                    window.remove_window();
                                }
                            },
                        )),
                    ),
            )
            .child(
                div()
                    .id("journal-reading-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p_5()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .when(day.plans.is_empty(), |el| {
                        el.child(ui::panel().p_5().flex_shrink_0().child(ui::muted(
                            "这一天还没有完整预案。请在主窗口编辑或引用研究草稿。",
                        )))
                    })
                    .children(day.plans.iter().map(|plan| {
                        ui::panel()
                            .p_4()
                            .flex_shrink_0()
                            .flex()
                            .flex_col()
                            .gap_3()
                            .child(
                                div()
                                    .text_size(px(19.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(plan.sector.clone()),
                            )
                            .child(ui::muted(plan.basis.clone()))
                            .child(
                                div()
                                    .flex()
                                    .gap_5()
                                    .child(
                                        div()
                                            .flex_1()
                                            .flex()
                                            .flex_col()
                                            .gap_2()
                                            .child(ui::tag(format!("买入 · {}", plan.method), TEAL))
                                            .child(text_block("触发条件", &plan.entry, TEAL))
                                            .child(text_block("放弃条件", &plan.avoid, MUTED)),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .flex()
                                            .flex_col()
                                            .gap_2()
                                            .child(ui::tag(
                                                format!("卖出 · {}", plan.sell_method),
                                                UP,
                                            ))
                                            .child(text_block("离场条件", &plan.exit, UP))
                                            .child(text_block("继续持有", &plan.hold, MUTED)),
                                    ),
                            )
                            .child(ui::muted(format!("来源：{}", plan.source)))
                    }))
                    .child(ui::panel().p_4().flex_shrink_0().child(multiline(
                        "盘中观察与复盘笔记 · 当前窗口独立输入",
                        "journal-reading-note",
                        &self.note,
                        150.,
                    )))
                    .when_some(self.note_error.clone(), |el, error| {
                        el.child(
                            div()
                                .id("journal-reading-note-error")
                                .test_support()
                                .text_color(rgb(UP))
                                .child(error),
                        )
                    }),
            )
    }
}

#[cfg(feature = "test-support")]
mod test_support;
#[cfg(feature = "test-support")]
pub(super) use test_support::verify_rejected_notes;
