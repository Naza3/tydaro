//! Native review notebook. Business documents live in DesktopState; inputs and
//! focus handles belong to one native window only.
use crate::app::state::{
    self as desktop_state, Command, CommandResult, DesktopEvent, DesktopState, Draft, PageId, Plan,
    SourceContext,
};
use crate::app::ui::{self as ui, BG, DOWN, INK, LINE, MUTED, TEAL, UP};
use crate::market::Symbol;
use gpui_kit::{
    component::{
        Sizable, WindowExt,
        button::{Button, ButtonVariants},
        input::{Input, InputEvent, InputState, Textarea, TextareaState},
    },
    prelude::*,
    *,
};

const TODAY: &str = "2026-10-14";

struct Editor {
    draft: Draft,
    sector: Entity<InputState>,
    method: Entity<InputState>,
    sell_method: Entity<InputState>,
    basis: Entity<TextareaState>,
    entry: Entity<TextareaState>,
    avoid: Entity<TextareaState>,
    exit: Entity<TextareaState>,
    hold: Entity<TextareaState>,
    subscriptions: Vec<Subscription>,
    scroll: ScrollHandle,
}
#[derive(Clone, Copy)]
enum FormKind {
    Hotspot,
    Buy,
}
struct MethodsEditor {
    buy: Entity<TextareaState>,
    sell: Entity<TextareaState>,
}
struct RecordForm {
    kind: FormKind,
    id: Option<String>,
    fields: Vec<Entity<InputState>>,
    narrative: Entity<TextareaState>,
}
#[derive(Clone)]
enum DeleteTarget {
    Plan(String, String),
    Draft(String, String),
    Hotspot(String),
    Buy(String),
}

pub(crate) struct JournalPage {
    date: String,
    date_input: Entity<InputState>,
    note: Entity<TextareaState>,
    note_synced: String,
    note_error: Option<String>,
    note_subscription: Option<Subscription>,
    editor: Option<Editor>,
    form: Option<RecordForm>,
    methods_editor: Option<MethodsEditor>,
    buys_history: bool,
    delete: Option<DeleteTarget>,
    message: Option<String>,
    error: Option<String>,
    focus: FocusHandle,
}
impl EventEmitter<DesktopEvent> for JournalPage {}

fn input(
    value: impl Into<SharedString>,
    placeholder: &'static str,
    window: &mut Window,
    cx: &mut App,
) -> Entity<InputState> {
    let value = value.into();
    cx.new(|cx| {
        InputState::new(window, cx)
            .default_value(value)
            .placeholder(placeholder)
    })
}
fn area(
    value: impl Into<SharedString>,
    placeholder: &'static str,
    window: &mut Window,
    cx: &mut App,
) -> Entity<TextareaState> {
    let value = value.into();
    cx.new(|cx| {
        TextareaState::new(window, cx)
            .default_value(value)
            .auto_grow(2, 3)
            .placeholder(placeholder)
    })
}
/// Rendering can run before a queued InputEvent::Change. Compare shared data
/// to the last synchronized snapshot, not to the live editor buffer: otherwise
/// a repaint replaces new keystrokes with stale global text. External updates
/// still reach the other window once its own pending input has been published.
fn sync_shared_note(
    note: &Entity<TextareaState>,
    synced: &mut String,
    shared: &str,
    window: &mut Window,
    cx: &mut App,
) {
    if shared == synced.as_str() {
        return;
    }
    let local = note.read(cx).value();
    if local.as_ref() != synced.as_str() && local.as_ref() != shared {
        return;
    }
    if local.as_ref() != shared {
        note.update(cx, |input, cx| {
            input.set_value(shared.to_string(), window, cx)
        });
    }
    *synced = shared.to_string();
}

fn publish_note(
    note: &Entity<TextareaState>,
    synced: &mut String,
    date: &str,
    key: String,
    cx: &mut App,
) -> anyhow::Result<()> {
    let value = note.read(cx).value().to_string();
    if value == *synced {
        desktop_state::report_pending_input(cx, key, None);
        return Ok(());
    }
    match desktop_state::dispatch(
        cx,
        Command::SetNote {
            date: date.to_string(),
            text: value.clone(),
        },
    ) {
        Ok(_) => {
            *synced = value;
            desktop_state::report_pending_input(cx, key, None);
            Ok(())
        }
        Err(error) => {
            desktop_state::report_pending_input(cx, key, Some(error.to_string()));
            Err(error)
        }
    }
}

fn action(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Button {
    Button::new(id).label(label).small()
}
fn field(label: &'static str, id: &'static str, state: &Entity<InputState>) -> Div {
    div()
        .flex()
        .flex_col()
        .flex_shrink_0()
        .gap_1()
        .min_w_0()
        .child(ui::muted(label))
        .child(Input::new(state).id(id).w_full())
}
fn multiline(
    label: &'static str,
    id: &'static str,
    state: &Entity<TextareaState>,
    height: f32,
) -> AnyElement {
    div()
        .id(id)
        .test_support()
        .flex()
        .flex_col()
        .flex_shrink_0()
        .gap_1()
        .min_w_0()
        .child(ui::muted(label))
        .child(
            Textarea::new(state)
                .accessibility_id(format!("{id}-input"))
                .aria_label(label)
                .h(px(height))
                .w_full(),
        )
        .into_any_element()
}
fn plan_context(plan: &Plan) -> SourceContext {
    SourceContext {
        date: plan.date.clone(),
        sector: plan.sector.clone(),
        symbols: plan.symbols.clone(),
        reason: plan.basis.clone(),
        origin: PageId::Journal,
    }
}
fn text_block(label: &'static str, value: &str, color: u32) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .min_w_0()
        .child(div().text_size(px(11.)).text_color(rgb(color)).child(label))
        .child(div().text_size(px(13.)).line_height(relative(1.5)).child(
            if value.trim().is_empty() {
                "未填写".to_string()
            } else {
                value.to_string()
            },
        ))
}

mod editor;
mod editor_view;
mod reading;
mod record_view;
mod records;
mod render;

#[cfg(feature = "test-support")]
mod test_support;
#[cfg(feature = "test-support")]
pub(crate) use test_support::smoke_rejected_inputs;

impl JournalPage {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let date = TODAY.to_string();
        let note_text = cx
            .global::<DesktopState>()
            .journal
            .days
            .get(&date)
            .map(|day| day.note.clone())
            .unwrap_or_default();
        let note = area(
            note_text.clone(),
            "记录盘中观察、执行与复盘，不自动生成交易决策",
            window,
            cx,
        );
        let subscription = cx.subscribe(&note, move |this, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                this.persist_note(cx);
                cx.notify();
            }
        });
        Self {
            date: date.clone(),
            date_input: input(date, "YYYY-MM-DD", window, cx),
            note,
            note_synced: note_text,
            note_error: None,
            note_subscription: Some(subscription),
            editor: None,
            form: None,
            methods_editor: None,
            buys_history: false,
            delete: None,
            message: None,
            error: None,
            focus: cx.focus_handle(),
        }
    }

    pub(crate) fn is_editing(&self) -> bool {
        self.editor.is_some()
            || self.form.is_some()
            || self.methods_editor.is_some()
            || self.buys_history
            || self.delete.is_some()
    }

    pub(crate) fn receive_on_date(
        &mut self,
        context: SourceContext,
        date: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.persist_editor(cx) || !self.persist_note(cx) {
            return;
        }
        if !desktop_state::valid_date(&date) {
            self.error = Some("复盘日期无效，请使用 YYYY-MM-DD".into());
            cx.notify();
            return;
        }
        let draft_id = match desktop_state::dispatch(
            cx,
            Command::CreateDraft {
                context,
                date: date.clone(),
            },
        )
        .and_then(CommandResult::into_id)
        {
            Ok(id) => id,
            Err(error) => {
                self.error = Some(error.to_string());
                cx.notify();
                return;
            }
        };
        self.select_date(&date, window, cx);
        self.open_draft(&draft_id, window, cx);
        self.message = Some("已建立独立草稿，请补充买入或卖出条件后保存为预案".into());
    }

    pub(crate) fn receive_candidates(
        &mut self,
        set: tydaro_core::research::CandidateSet,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.persist_editor(cx) || !self.persist_note(cx) {
            return;
        }
        let date = set.applicable_date().as_str().to_owned();
        match desktop_state::dispatch(cx, Command::CreateCandidateDraft(set))
            .and_then(CommandResult::into_id)
        {
            Ok(id) => {
                self.select_date(&date, window, cx);
                self.open_draft(&id, window, cx);
                self.message = Some("已保存冻结的模拟候选依据，请补充次日触发与放弃条件。".into());
            }
            Err(error) => {
                self.error = Some(error.to_string());
                cx.notify();
            }
        }
    }

    fn select_date(&mut self, date: &str, window: &mut Window, cx: &mut Context<Self>) {
        if !self.persist_editor(cx) || !self.persist_note(cx) {
            return;
        }
        self.editor = None;
        self.date = date.to_string();
        self.date_input.update(cx, |state, cx| {
            state.set_value(date.to_string(), window, cx)
        });
        let text = cx
            .global::<DesktopState>()
            .journal
            .days
            .get(date)
            .map(|day| day.note.clone())
            .unwrap_or_default();
        self.note_subscription = None;
        self.note_synced = text.clone();
        self.note = area(text, "记录盘中观察、执行与复盘", window, cx);
        self.note_error = None;
        self.note_subscription = Some(cx.subscribe(
            &self.note,
            move |this, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    this.persist_note(cx);
                    cx.notify();
                }
            },
        ));
        self.error = None;
        cx.notify();
    }

    fn persist_note(&mut self, cx: &mut Context<Self>) -> bool {
        let key = format!("journal-note:{:?}", cx.entity_id());
        self.note_error = publish_note(&self.note, &mut self.note_synced, &self.date, key, cx)
            .err()
            .map(|error| format!("笔记尚未保存：{error}"));
        self.note_error.is_none()
    }
}
