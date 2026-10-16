//! Native review notebook. Business documents live in DesktopState; inputs and
//! focus handles belong to one native window only.
use crate::{
    desktop_state::{
        self, Command, CommandResult, DesktopEvent, DesktopState, Draft, PageId, Plan,
        SourceContext,
    },
    desktop_ui::{self as ui, BG, DOWN, INK, LINE, MUTED, TEAL, UP},
    market::Symbol,
};
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

pub struct JournalPage {
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

impl JournalPage {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
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
    pub fn is_editing(&self) -> bool {
        self.editor.is_some()
            || self.form.is_some()
            || self.methods_editor.is_some()
            || self.buys_history
            || self.delete.is_some()
    }
    pub fn receive_on_date(
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
    pub fn receive_candidates(
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
    fn create(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.persist_editor(cx) {
            return;
        }
        let date = self.date.clone();
        let id = match desktop_state::dispatch(cx, Command::NewDraft { date })
            .and_then(CommandResult::into_id)
        {
            Ok(id) => id,
            Err(error) => {
                self.error = Some(error.to_string());
                cx.notify();
                return;
            }
        };
        self.open_draft(&id, window, cx);
    }
    fn open_draft(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        let draft = cx
            .global::<DesktopState>()
            .journal
            .days
            .get(&self.date)
            .and_then(|day| day.drafts.iter().find(|draft| draft.plan.id == id))
            .cloned();
        if let Some(draft) = draft {
            self.begin_editor(draft, window, cx);
        }
    }
    fn edit_plan(&mut self, plan: Plan, window: &mut Window, cx: &mut Context<Self>) {
        if !self.persist_editor(cx) {
            return;
        }
        match desktop_state::dispatch(
            cx,
            Command::EditPlan {
                date: plan.date,
                id: plan.id,
            },
        )
        .and_then(CommandResult::into_draft)
        {
            Ok(draft) => self.begin_editor(draft, window, cx),
            Err(error) => {
                self.error = Some(error.to_string());
                cx.notify();
            }
        }
    }

    fn begin_editor(&mut self, draft: Draft, window: &mut Window, cx: &mut Context<Self>) {
        let p = &draft.plan;
        let mut editor = Editor {
            sector: input(p.sector.clone(), "板块 / 研究主题", window, cx),
            method: input(p.method.clone(), "如：分歧转一致", window, cx),
            sell_method: input(p.sell_method.clone(), "如：冲高分批", window, cx),
            basis: area(
                p.basis.clone(),
                "写下事实、依据与仍待确认的条件",
                window,
                cx,
            ),
            entry: area(p.entry.clone(), "什么条件出现才考虑买入", window, cx),
            avoid: area(p.avoid.clone(), "什么情况放弃买入", window, cx),
            exit: area(p.exit.clone(), "什么条件出现时离场", window, cx),
            hold: area(p.hold.clone(), "哪些条件仍然支持持有", window, cx),
            draft,
            subscriptions: Vec::new(),
            scroll: ScrollHandle::new(),
        };
        for entity in [&editor.sector, &editor.method, &editor.sell_method] {
            editor
                .subscriptions
                .push(cx.subscribe(entity, |this, _, event: &InputEvent, cx| {
                    if matches!(event, InputEvent::Change) {
                        this.persist_editor(cx);
                        cx.notify();
                    }
                }));
        }
        for entity in [
            &editor.basis,
            &editor.entry,
            &editor.avoid,
            &editor.exit,
            &editor.hold,
        ] {
            editor
                .subscriptions
                .push(cx.subscribe(entity, |this, _, event: &InputEvent, cx| {
                    if matches!(event, InputEvent::Change) {
                        this.persist_editor(cx);
                        cx.notify();
                    }
                }));
        }
        editor
            .sector
            .update(cx, |state, cx| state.focus(window, cx));
        self.editor = Some(editor);
        self.form = None;
        self.error = None;
        cx.notify();
    }
    fn editor_draft(&self, cx: &App) -> Option<Draft> {
        let e = self.editor.as_ref()?;
        let mut draft = e.draft.clone();
        let p = &mut draft.plan;
        p.sector = e.sector.read(cx).value().trim().to_string();
        p.method = e.method.read(cx).value().trim().to_string();
        p.sell_method = e.sell_method.read(cx).value().trim().to_string();
        p.basis = e.basis.read(cx).value().to_string();
        p.entry = e.entry.read(cx).value().to_string();
        p.avoid = e.avoid.read(cx).value().to_string();
        p.exit = e.exit.read(cx).value().to_string();
        p.hold = e.hold.read(cx).value().to_string();
        Some(draft)
    }
    fn persist_note(&mut self, cx: &mut Context<Self>) -> bool {
        let key = format!("journal-note:{:?}", cx.entity_id());
        self.note_error = publish_note(&self.note, &mut self.note_synced, &self.date, key, cx)
            .err()
            .map(|error| format!("笔记尚未保存：{error}"));
        self.note_error.is_none()
    }

    fn persist_editor(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(draft) = self.editor_draft(cx) else {
            return true;
        };
        let key = format!("journal-draft:{:?}", cx.entity_id());
        match desktop_state::dispatch(cx, Command::UpsertDraft(draft)) {
            Ok(_) => {
                desktop_state::report_pending_input(cx, key, None);
                self.error = None;
                true
            }
            Err(error) => {
                desktop_state::report_pending_input(cx, key, Some(error.to_string()));
                self.error = Some(format!("草稿尚未保存：{error}"));
                cx.notify();
                false
            }
        }
    }

    fn close_editor(
        &mut self,
        retained_message: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.persist_editor(cx) {
            return;
        }
        self.editor = None;
        self.error = None;
        if retained_message {
            self.message = Some("草稿已保留，可在当天继续编辑".into());
        }
        window.focus(&self.focus, cx);
        cx.notify();
    }

    fn save_plan(&mut self, cx: &mut Context<Self>) {
        if !self.persist_editor(cx) {
            return;
        }
        let Some(draft) = self.editor_draft(cx) else {
            return;
        };
        if let Err(error) = draft.plan.validate_complete() {
            self.error = Some(error.to_string());
            cx.notify();
            return;
        }
        let result = desktop_state::dispatch(cx, Command::SavePlanDraft(draft));
        match result {
            Ok(_) => {
                self.editor = None;
                self.error = None;
                self.message = Some("预案已更新；原有记录按身份更新，不重复追加".into());
            }
            Err(error) => self.error = Some(error.to_string()),
        }
        cx.notify();
    }
    fn toggle_symbol(&mut self, symbol: Symbol, cx: &mut Context<Self>) {
        if let Some(e) = &mut self.editor {
            if e.draft.plan.symbols.contains(&symbol) {
                e.draft.plan.symbols.retain(|item| *item != symbol);
            } else {
                e.draft.plan.symbols.push(symbol);
            }
        }
        self.persist_editor(cx);
        cx.notify();
    }
    fn start_form(
        &mut self,
        kind: FormKind,
        id: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let state = cx.global::<DesktopState>();
        let values = match kind {
            FormKind::Hotspot => id
                .as_ref()
                .and_then(|id| state.journal.hotspots.iter().find(|item| &item.id == id))
                .map(|item| vec![item.date.clone(), item.sector.clone(), item.heights.clone()])
                .unwrap_or_else(|| vec![self.date.clone(), String::new(), String::new()]),
            FormKind::Buy => id
                .as_ref()
                .and_then(|id| state.journal.buys.iter().find(|item| &item.id == id))
                .map(|item| {
                    vec![
                        item.date.clone(),
                        item.stock.clone(),
                        item.sector.clone(),
                        item.method.clone(),
                        item.profit.to_string(),
                        item.note.clone(),
                    ]
                })
                .unwrap_or_else(|| {
                    vec![
                        self.date.clone(),
                        String::new(),
                        String::new(),
                        String::new(),
                        String::new(),
                        String::new(),
                    ]
                }),
        };
        // Heights and notes are multiline documents. A single-line InputState
        // normalizes newlines away even before any edit, so keep their own area.
        let narrative = area(
            values.last().cloned().unwrap_or_default(),
            "可分行记录",
            window,
            cx,
        );
        let fields: Vec<_> = values
            .into_iter()
            .map(|value| input(value, "", window, cx))
            .collect();
        // Modal input belongs to this window. Move focus away from the plan
        // underneath so typing immediately after opening edits this record.
        fields[1].update(cx, |input, cx| input.focus(window, cx));
        self.form = Some(RecordForm {
            kind,
            id,
            fields,
            narrative,
        });
        self.error = None;
        cx.notify();
    }
    fn save_form(&mut self, cx: &mut Context<Self>) {
        let Some(form) = &self.form else { return };
        let mut values: Vec<String> = form
            .fields
            .iter()
            .map(|field| field.read(cx).value().trim().to_string())
            .collect();
        if let Some(narrative) = values.last_mut() {
            *narrative = form.narrative.read(cx).value().to_string();
        }
        if !desktop_state::valid_date(&values[0]) {
            self.error = Some("请填写有效日期 YYYY-MM-DD".into());
            cx.notify();
            return;
        }
        if values[1].is_empty() {
            self.error = Some("请填写名称".into());
            cx.notify();
            return;
        }
        let kind = form.kind;
        let original_id = form.id.clone();
        let profit = if matches!(kind, FormKind::Buy) {
            match values[4]
                .parse::<f64>()
                .ok()
                .filter(|value| value.is_finite())
            {
                Some(value) => value,
                None => {
                    self.error = Some("收益率须为有限数字，例如 10 或 -2.4".into());
                    cx.notify();
                    return;
                }
            }
        } else {
            0.
        };
        let command = match kind {
            FormKind::Hotspot => Command::SaveHotspot {
                id: original_id,
                date: values[0].clone(),
                sector: values[1].clone(),
                heights: values[2].clone(),
            },
            FormKind::Buy => Command::SaveBuy {
                id: original_id,
                date: values[0].clone(),
                stock: values[1].clone(),
                sector: values[2].clone(),
                method: values[3].clone(),
                profit,
                note: values[5].clone(),
            },
        };
        match desktop_state::dispatch(cx, command) {
            Ok(_) => {
                self.form = None;
                self.error = None;
                self.message = Some("手工记录已更新，独立于行情和账户数据".into());
            }
            Err(error) => self.error = Some(error.to_string()),
        }
        cx.notify();
    }
    fn confirm_delete(&mut self, cx: &mut Context<Self>) {
        let Some(target) = self.delete.take() else {
            return;
        };
        let command = match target {
            DeleteTarget::Plan(date, id) => Command::DeletePlan { date, id },
            DeleteTarget::Draft(date, id) => Command::DeleteDraft { date, id },
            DeleteTarget::Hotspot(id) => Command::DeleteHotspot(id),
            DeleteTarget::Buy(id) => Command::DeleteBuy(id),
        };
        if let Err(error) = desktop_state::dispatch(cx, command) {
            self.error = Some(error.to_string());
            cx.notify();
            return;
        }
        self.message = Some("已删除选中的记录".into());
        cx.notify();
    }
    fn open_reading(&mut self, source: &Window, cx: &mut Context<Self>) {
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
    fn plan_card(&self, plan: &Plan, cx: &mut Context<Self>) -> AnyElement {
        let editing = plan.clone();
        let watch = plan_context(plan);
        let delete_date = plan.date.clone();
        let delete_id = plan.id.clone();
        div()
            .id(SharedString::from(format!("journal-plan-{}", plan.id)))
            .test_support()
            .p_4()
            .flex()
            .flex_col()
            .flex_shrink_0()
            .gap_3()
            .bg(rgb(0xffffff))
            .border_1()
            .border_color(rgb(LINE))
            .rounded_lg()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(17.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(plan.sector.clone()),
                    )
                    .child(ui::tag(
                        if plan.method.is_empty() {
                            "卖出计划"
                        } else {
                            "买卖预案"
                        },
                        TEAL,
                    ))
                    .child(
                        action(
                            SharedString::from(format!("journal-edit-{}", plan.id)),
                            "编辑",
                        )
                        .ghost()
                        .on_click(cx.listener(
                            move |this, _, window, cx| this.edit_plan(editing.clone(), window, cx),
                        )),
                    )
                    .child(
                        action(
                            SharedString::from(format!("journal-delete-{}", plan.id)),
                            "删除",
                        )
                        .ghost()
                        .text_color(rgb(MUTED))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.delete =
                                Some(DeleteTarget::Plan(delete_date.clone(), delete_id.clone()));
                            cx.notify();
                        })),
                    ),
            )
            .child(div().text_size(px(13.)).text_color(rgb(MUTED)).child(
                if plan.basis.is_empty() {
                    "尚未填写研究依据".to_string()
                } else {
                    plan.basis.clone()
                },
            ))
            .child(
                div()
                    .flex()
                    .gap_5()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(ui::tag(
                                format!(
                                    "买入 · {}",
                                    if plan.method.is_empty() {
                                        "未计划"
                                    } else {
                                        &plan.method
                                    }
                                ),
                                TEAL,
                            ))
                            .child(text_block("触发条件", &plan.entry, TEAL))
                            .child(text_block("放弃条件", &plan.avoid, MUTED)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(ui::tag(
                                format!(
                                    "卖出 · {}",
                                    if plan.sell_method.is_empty() {
                                        "未计划"
                                    } else {
                                        &plan.sell_method
                                    }
                                ),
                                UP,
                            ))
                            .child(text_block("离场条件", &plan.exit, UP))
                            .child(text_block("继续持有", &plan.hold, MUTED)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_2()
                    .pt_2()
                    .border_t_1()
                    .border_color(rgb(LINE))
                    .children(plan.symbols.iter().map(|symbol| {
                        let symbol = *symbol;
                        let mut context = plan_context(plan);
                        context.symbols = vec![symbol];
                        action(
                            SharedString::from(format!(
                                "journal-symbol-{}-{}",
                                plan.id,
                                symbol.code()
                            )),
                            format!("{} {}", symbol.name(), symbol.code()),
                        )
                        .ghost()
                        .text_color(rgb(TEAL))
                        .on_click(cx.listener(move |_, _, _, cx| {
                            cx.emit(DesktopEvent::Inspect(context.clone()))
                        }))
                    }))
                    .child(div().flex_1())
                    .child(ui::muted(plan.source.clone()))
                    .child(
                        action(
                            SharedString::from(format!("journal-watch-{}", plan.id)),
                            "加入盯盘 ↗",
                        )
                        .ghost()
                        .text_color(rgb(TEAL))
                        .on_click(cx.listener(move |_, _, _, cx| {
                            cx.emit(DesktopEvent::AddToWatch(watch.clone()))
                        })),
                    ),
            )
            .into_any_element()
    }
    fn editor_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(e) = &self.editor else {
            return div().into_any_element();
        };
        let selected = e.draft.plan.symbols.clone();
        div().id("journal-editor").test_support().absolute().inset_0().occlude().bg(rgba(0x23334838)).flex().justify_end()
            .child(div().w(px(850.)).h_full().min_h_0().flex().flex_col().bg(rgb(0xffffff)).border_l_1().border_color(rgb(LINE))
                .child(div().p_5().flex_shrink_0().flex().items_center().gap_3().border_b_1().border_color(rgb(LINE))
                    .child(div().flex_1().child(div().text_size(px(21.)).font_weight(FontWeight::SEMIBOLD).child(if e.draft.editing_id.is_some() { "编辑预案" } else { "新的研究预案" }))
                        .child(ui::muted(format!("{} · 草稿自动保留，完整预案需要明确触发条件", e.draft.plan.date))))
                    .child(action("journal-editor-close", "关闭").ghost().on_click(cx.listener(|this, _, window, cx| { this.close_editor(false, window, cx); }))))
                .child(div().id("journal-editor-scroll").flex_1().min_h_0().overflow_y_scroll().track_scroll(&e.scroll).p_5().flex().flex_col().gap_4()
                    .child(field("板块 / 主题 *", "journal-sector", &e.sector))
                    .child(multiline("研究依据", "journal-basis", &e.basis, 72.))
                    .child(div().flex().flex_col().flex_shrink_0().gap_2().child(ui::muted("关联股票 · 可多选"))
                        .child(div().flex().flex_wrap().gap_1().children(Symbol::ALL.iter().filter(|symbol| symbol.currency() == "CNY").map(|symbol| {
                            let symbol = *symbol;
                            action(SharedString::from(format!("journal-pick-{}", symbol.code())), symbol.name()).ghost()
                                .bg(rgb(if selected.contains(&symbol) { 0xe2f4f2 } else { 0xf5f7f9 })).text_color(rgb(if selected.contains(&symbol) { TEAL } else { MUTED }))
                                .on_click(cx.listener(move |this, _, _, cx| this.toggle_symbol(symbol, cx)))
                        }))))
                    .child(div().flex().flex_shrink_0().items_center().child(ui::muted("方法可以自由输入，也可以选取你维护的常用方法。"))
                        .child(div().flex_1()).child(action("journal-methods-open", "管理方法词库").ghost().text_color(rgb(TEAL))
                            .on_click(cx.listener(|this, _, window, cx| this.open_methods(window, cx)))))
                    .child(div().flex().flex_shrink_0().gap_4()
                        .child(ui::panel().flex_1().p_4().flex().flex_col().gap_3().child(ui::tag("买入计划", TEAL))
                            .child(field("买入方法", "journal-method", &e.method))
                            .child(self.method_chips(false, cx))
                            .child(multiline("买入触发条件", "journal-entry", &e.entry, 85.))
                            .child(multiline("放弃买入条件", "journal-avoid", &e.avoid, 72.)))
                        .child(ui::panel().flex_1().p_4().flex().flex_col().gap_3().child(ui::tag("卖出计划", UP))
                            .child(field("卖出方法", "journal-sell-method", &e.sell_method))
                            .child(self.method_chips(true, cx))
                            .child(multiline("卖出触发条件", "journal-exit", &e.exit, 85.))
                            .child(multiline("继续持有条件", "journal-hold", &e.hold, 72.))))
                    .child(ui::muted(format!("来源：{}", e.draft.plan.source)).flex_shrink_0())
                    .child(ui::muted("允许只写买入或只写卖出。已填写的一侧须同时有方法和触发条件；未完成内容请保留草稿。").flex_shrink_0()))
                .child(div().p_4().flex_shrink_0().border_t_1().border_color(rgb(LINE)).flex().items_center().gap_2()
                    .child(div().flex_1().text_size(px(12.)).text_color(rgb(UP)).child(self.error.clone().unwrap_or_default()))
                    .child(action("journal-draft-save", "保留草稿").on_click(cx.listener(|this, _, window, cx| { this.close_editor(true, window, cx); })))
                    .child(action("journal-plan-save", "保存为预案").primary().on_click(cx.listener(|this, _, window, cx| { this.save_plan(cx); if this.editor.is_none() { window.focus(&this.focus, cx); } }))))).into_any_element()
    }
    fn form_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(form) = &self.form else {
            return div().into_any_element();
        };
        let labels: &[&str] = match form.kind {
            FormKind::Hotspot => &["日期", "热点名称", "高度与观察"],
            FormKind::Buy => &[
                "买入日期",
                "股票名称",
                "板块",
                "方法",
                "手工收益率（%）",
                "备注",
            ],
        };
        let ids = [
            "journal-record-date",
            "journal-record-name",
            "journal-record-sector",
            "journal-record-method",
            "journal-record-profit",
            "journal-record-note",
        ];
        let title = match form.kind {
            FormKind::Hotspot => "人工热点",
            FormKind::Buy => "手工买入记录",
        };
        div()
            .absolute()
            .inset_0()
            .occlude()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgba(0x23334838))
            .child(
                ui::panel()
                    .w(px(580.))
                    .p_5()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(
                        div()
                            .text_size(px(21.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(title),
                    )
                    .child(ui::muted(
                        "内容由你手工维护；不与题材分析、行情或账户成交自动同步。",
                    ))
                    .children(
                        form.fields
                            .iter()
                            .take(form.fields.len().saturating_sub(1))
                            .enumerate()
                            .map(|(index, value)| field(labels[index], ids[index], value)),
                    )
                    .child(multiline(
                        if matches!(form.kind, FormKind::Hotspot) {
                            "高度与观察"
                        } else {
                            "备注"
                        },
                        "journal-record-description",
                        &form.narrative,
                        80.,
                    ))
                    .child(
                        div()
                            .text_color(rgb(UP))
                            .text_size(px(12.))
                            .child(self.error.clone().unwrap_or_default()),
                    )
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .gap_2()
                            .child(
                                action("journal-record-cancel", "取消").on_click(cx.listener(
                                    |this, _, _, cx| {
                                        this.form = None;
                                        this.error = None;
                                        cx.notify();
                                    },
                                )),
                            )
                            .child(
                                action("journal-record-save", "保存记录")
                                    .primary()
                                    .on_click(cx.listener(|this, _, _, cx| this.save_form(cx))),
                            ),
                    ),
            )
            .into_any_element()
    }
    fn open_methods(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let journal = &cx.global::<DesktopState>().journal;
        let buy = journal.methods.join("\n");
        let sell = journal.sell_methods.join("\n");
        let editor = MethodsEditor {
            buy: area(buy, "每行一个买入方法", window, cx),
            sell: area(sell, "每行一个卖出方法", window, cx),
        };
        editor.buy.update(cx, |input, cx| input.focus(window, cx));
        self.methods_editor = Some(editor);
        self.error = None;
        cx.notify();
    }
    fn save_methods(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = &self.methods_editor else {
            return;
        };
        let parse = |text: &str| -> Result<Vec<String>, &'static str> {
            let mut methods = Vec::new();
            for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
                if line.len() > 512 || line.chars().any(char::is_control) {
                    return Err("每个方法不超过 512 字节，且不能包含控制字符");
                }
                if !methods.iter().any(|method| method == line) {
                    methods.push(line.to_string());
                }
            }
            if methods.len() > 128 {
                return Err("买入和卖出方法各最多 128 个");
            }
            Ok(methods)
        };
        let buy = parse(editor.buy.read(cx).value().as_ref());
        let sell = parse(editor.sell.read(cx).value().as_ref());
        match (buy, sell) {
            (Ok(buy), Ok(sell)) => {
                if let Err(error) = desktop_state::dispatch(cx, Command::SetMethods { buy, sell }) {
                    self.error = Some(error.to_string());
                    cx.notify();
                    return;
                }
                self.methods_editor = None;
                self.error = None;
                self.message = Some("方法词库已更新；已有预案的文字保持原样".into());
            }
            (Err(error), _) | (_, Err(error)) => self.error = Some(error.to_string()),
        }
        cx.notify();
    }
    fn method_chips(&self, sell: bool, cx: &mut Context<Self>) -> Div {
        let journal = &cx.global::<DesktopState>().journal;
        let methods = if sell {
            journal.sell_methods.clone()
        } else {
            journal.methods.clone()
        };
        div()
            .flex()
            .flex_wrap()
            .gap_1()
            .children(methods.iter().take(6).enumerate().map(|(index, method)| {
                let value = method.clone();
                action(
                    SharedString::from(format!("journal-method-chip-{sell}-{index}")),
                    method.clone(),
                )
                .ghost()
                .text_color(rgb(TEAL))
                .on_click(cx.listener(move |this, _, window, cx| {
                    if let Some(editor) = &this.editor {
                        let entity = if sell {
                            editor.sell_method.clone()
                        } else {
                            editor.method.clone()
                        };
                        entity.update(cx, |input, cx| input.set_value(value.clone(), window, cx));
                        this.persist_editor(cx);
                    }
                    cx.notify();
                }))
            }))
    }
    fn methods_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(editor) = &self.methods_editor else {
            return div().into_any_element();
        };
        div()
            .id("journal-methods-editor")
            .test_support()
            .absolute()
            .inset_0()
            .occlude()
            .bg(rgba(0x23334850))
            .flex()
            .items_center()
            .justify_center()
            .child(
                ui::panel()
                    .w(px(690.))
                    .p_5()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(
                        div()
                            .text_size(px(21.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("管理买卖方法"),
                    )
                    .child(ui::muted(
                        "每行一个方法，保存时去除空行并去重。修改词库不会重写已保存的预案。",
                    ))
                    .child(
                        div()
                            .flex()
                            .gap_4()
                            .child(div().flex_1().min_w_0().child(multiline(
                                "买入方法",
                                "journal-buy-methods",
                                &editor.buy,
                                230.,
                            )))
                            .child(div().flex_1().min_w_0().child(multiline(
                                "卖出方法",
                                "journal-sell-methods",
                                &editor.sell,
                                230.,
                            ))),
                    )
                    .child(
                        div()
                            .text_color(rgb(UP))
                            .text_size(px(12.))
                            .child(self.error.clone().unwrap_or_default()),
                    )
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .gap_2()
                            .child(
                                action("journal-methods-cancel", "取消").on_click(cx.listener(
                                    |this, _, _, cx| {
                                        this.methods_editor = None;
                                        this.error = None;
                                        cx.notify();
                                    },
                                )),
                            )
                            .child(
                                action("journal-methods-save", "保存词库")
                                    .primary()
                                    .on_click(cx.listener(|this, _, _, cx| this.save_methods(cx))),
                            ),
                    ),
            )
            .into_any_element()
    }
    fn buys_history_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut buys = cx.global::<DesktopState>().journal.buys.clone();
        buys.sort_by(|a, b| b.date.cmp(&a.date).then(a.id.cmp(&b.id)));
        div()
            .id("journal-buy-history-view")
            .test_support()
            .absolute()
            .inset_0()
            .occlude()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgba(0x23334838))
            .child(
                ui::panel()
                    .w(px(850.))
                    .h(px(590.))
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .p_4()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .flex_1()
                                    .child(
                                        div()
                                            .text_size(px(21.))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child("全部手工买入记录"),
                                    )
                                    .child(ui::muted("按买入日期降序；手工收益率不代表账户绩效。")),
                            )
                            .child(
                                action("journal-buy-history-close", "关闭")
                                    .ghost()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.buys_history = false;
                                        cx.notify();
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .px_4()
                            .py_2()
                            .flex()
                            .gap_2()
                            .bg(rgb(BG))
                            .text_color(rgb(MUTED))
                            .text_size(px(12.))
                            .child(div().w(px(110.)).child("买入日期"))
                            .child(div().w(px(110.)).child("股票"))
                            .child(div().w(px(110.)).child("板块"))
                            .child(div().flex_1().child("方法"))
                            .child(div().w(px(80.)).child("手工收益"))
                            .child(div().w(px(100.)).child("操作")),
                    )
                    .child(
                        div()
                            .id("journal-buy-history-scroll")
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .children(buys.iter().map(|buy| {
                                let edit_id = buy.id.clone();
                                let delete_id = buy.id.clone();
                                div()
                                    .px_4()
                                    .py_2()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .border_b_1()
                                    .border_color(rgb(LINE))
                                    .text_size(px(13.))
                                    .child(div().w(px(110.)).child(buy.date.clone()))
                                    .child(div().w(px(110.)).child(buy.stock.clone()))
                                    .child(div().w(px(110.)).child(buy.sector.clone()))
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .truncate()
                                            .child(buy.method.clone()),
                                    )
                                    .child(
                                        div()
                                            .w(px(80.))
                                            .text_color(rgb(if buy.profit >= 0. {
                                                UP
                                            } else {
                                                DOWN
                                            }))
                                            .font_weight(if buy.profit > 10. {
                                                FontWeight::BOLD
                                            } else {
                                                FontWeight::NORMAL
                                            })
                                            .child(format!("{:+.2}%", buy.profit)),
                                    )
                                    .child(
                                        div()
                                            .w(px(100.))
                                            .flex()
                                            .gap_1()
                                            .child(
                                                action(
                                                    SharedString::from(format!(
                                                        "journal-history-buy-edit-{}",
                                                        buy.id
                                                    )),
                                                    "编辑",
                                                )
                                                .ghost()
                                                .on_click(cx.listener(
                                                    move |this, _, window, cx| {
                                                        this.start_form(
                                                            FormKind::Buy,
                                                            Some(edit_id.clone()),
                                                            window,
                                                            cx,
                                                        )
                                                    },
                                                )),
                                            )
                                            .child(
                                                action(
                                                    SharedString::from(format!(
                                                        "journal-history-buy-delete-{}",
                                                        buy.id
                                                    )),
                                                    "删除",
                                                )
                                                .ghost()
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    this.delete =
                                                        Some(DeleteTarget::Buy(delete_id.clone()));
                                                    cx.notify();
                                                })),
                                            ),
                                    )
                            })),
                    )
                    .child(
                        div()
                            .p_3()
                            .flex()
                            .items_center()
                            .child(ui::muted(format!("共 {} 条记录", buys.len())))
                            .child(div().flex_1())
                            .child(
                                action("journal-history-add-buy", "＋ 补录买入")
                                    .primary()
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.start_form(FormKind::Buy, None, window, cx)
                                    })),
                            ),
                    ),
            )
            .into_any_element()
    }
    fn delete_view(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .absolute()
            .inset_0()
            .occlude()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgba(0x23334838))
            .child(
                ui::panel()
                    .w(px(460.))
                    .p_5()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(
                        div()
                            .text_size(px(20.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("删除这条记录？"),
                    )
                    .child(ui::muted(
                        "删除会修改复盘文档。关闭阅读窗口只关闭视图，不会删除任何记录。",
                    ))
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .gap_2()
                            .child(
                                action("journal-delete-cancel", "保留").on_click(cx.listener(
                                    |this, _, _, cx| {
                                        this.delete = None;
                                        cx.notify();
                                    },
                                )),
                            )
                            .child(
                                action("journal-delete-confirm", "确认删除")
                                    .text_color(rgb(UP))
                                    .on_click(
                                        cx.listener(|this, _, _, cx| this.confirm_delete(cx)),
                                    ),
                            ),
                    ),
            )
            .into_any_element()
    }
}

impl Render for JournalPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let journal = cx.global::<DesktopState>().journal.clone();
        let day = journal.days.get(&self.date).cloned().unwrap_or_default();
        sync_shared_note(&self.note, &mut self.note_synced, &day.note, window, cx);
        let mut hotspots = journal.hotspots.clone();
        hotspots.sort_by(|a, b| b.date.cmp(&a.date).then(b.created.cmp(&a.created)));
        let buys = journal.recent_buys();
        let plans = day
            .plans
            .iter()
            .map(|plan| self.plan_card(plan, cx))
            .collect::<Vec<_>>();
        ui::page().id("journal-page").test_support().relative().track_focus(&self.focus)
            .child(div().flex().items_center().gap_4().flex_shrink_0()
                .child(div().flex_1().child(ui::heading("REVIEW / 从观察走向决策", "复盘", "先写条件，再看结果。把研究整理成可以回顾的预案。")))
                .child(div().w(px(145.)).child(Input::new(&self.date_input).id("journal-date-input").w_full()))
                .child(action("journal-date-apply", "切换日期").on_click(cx.listener(|this, _, window, cx| {
                    let date = this.date_input.read(cx).value().to_string();
                    if desktop_state::valid_date(&date) { this.select_date(&date, window, cx); }
                    else { this.error = Some("请输入有效日期 YYYY-MM-DD".into()); cx.notify(); }
                })))
                .child(action("journal-reading-open", "阅读窗口 ↗").on_click(cx.listener(|this, _, window, cx| this.open_reading(window, cx))))
                .child(action("journal-new-plan", "＋ 写预案").primary().on_click(cx.listener(|this, _, window, cx| this.create(window, cx)))))
            .child(div().flex_1().min_h_0().flex().gap_4()
                .child(div().flex_1().min_w_0().min_h_0().flex().flex_col().gap_3()
                    .child(ui::panel().p_3().flex_shrink_0().flex().flex_col().gap_2()
                        .child(div().flex().items_center().gap_2().child(div().flex_1().font_weight(FontWeight::SEMIBOLD).child("人工热点"))
                            .child(ui::muted(format!("{}/5 · 仅手工维护", hotspots.len())))
                            .child(action("journal-add-hotspot", "＋ 添加").ghost().text_color(rgb(TEAL)).on_click(cx.listener(|this, _, window, cx| this.start_form(FormKind::Hotspot, None, window, cx)))))
                        .child(div().flex().gap_2().children(hotspots.iter().map(|hot| {
                            let edit_id = hot.id.clone(); let delete_id = hot.id.clone();
                            div().flex_1().min_w_0().p_2().bg(rgb(0xf6f9fa)).rounded_md().flex().flex_col().gap_1()
                                .child(div().flex().items_center().child(div().flex_1().truncate().font_weight(FontWeight::MEDIUM).child(hot.sector.clone()))
                                    .child(action(SharedString::from(format!("journal-hot-delete-{}", hot.id)), "×").ghost().on_click(cx.listener(move |this, _, _, cx| { this.delete = Some(DeleteTarget::Hotspot(delete_id.clone())); cx.notify(); }))))
                                .child(ui::muted(hot.heights.clone()))
                                .child(action(SharedString::from(format!("journal-hot-edit-{}", hot.id)), format!("{} · 编辑", hot.date)).ghost().text_color(rgb(MUTED)).on_click(cx.listener(move |this, _, window, cx| this.start_form(FormKind::Hotspot, Some(edit_id.clone()), window, cx))))
                        }))))
                    .child(div().flex().items_center().gap_2().child(div().text_size(px(17.)).font_weight(FontWeight::SEMIBOLD).child(format!("{} 的预案", self.date)))
                        .child(ui::tag(format!("{} 条预案", day.plans.len()), TEAL)).child(ui::muted(format!("{} 份草稿", day.drafts.len()))))
                    .child(div().id("journal-document-scroll").test_support().flex_1().min_h_0().overflow_y_scroll().flex().flex_col().gap_3()
                        .when(day.plans.is_empty() && day.drafts.is_empty(), |el| el.child(ui::panel().p_5().child(ui::muted("这一天还没有预案。可以手工新建，也可以从题材或涨停研究中引用草稿。"))))
                        .children(day.drafts.iter().map(|draft| {
                            let open_id = draft.plan.id.clone(); let remove_id = draft.plan.id.clone(); let date = draft.plan.date.clone();
                            div().p_3().flex_shrink_0().border_1().border_color(rgb(0xcfe4e2)).bg(rgb(0xf2fbfa)).rounded_md().flex().items_center().gap_2()
                                .child(ui::tag("草稿", TEAL)).child(div().flex_1().child(if draft.plan.sector.is_empty() { "未命名研究预案".into() } else { draft.plan.sector.clone() }))
                                .child(action(SharedString::from(format!("journal-draft-open-{}", draft.plan.id)), "继续编辑").ghost().text_color(rgb(TEAL)).on_click(cx.listener(move |this, _, window, cx| this.open_draft(&open_id, window, cx))))
                                .child(action(SharedString::from(format!("journal-draft-delete-{}", draft.plan.id)), "删除").ghost().on_click(cx.listener(move |this, _, _, cx| { this.delete = Some(DeleteTarget::Draft(date.clone(), remove_id.clone())); cx.notify(); })))
                        }))
                        .children(plans)
                        .child(ui::panel().p_4().flex_shrink_0().flex().flex_col().gap_2().child(div().font_weight(FontWeight::SEMIBOLD).child("盘中观察与复盘笔记"))
                            .child(ui::muted("自动保存到当前日期。阅读窗口同步内容，窗口布局彼此独立。"))
                            .child(multiline("记录", "journal-note", &self.note, 120.)))))
                .child(div().w(px(260.)).flex_shrink_0().min_h_0().flex().flex_col().gap_3()
                    .child(ui::panel().p_3().flex().flex_col().gap_2()
                        .child(div().font_weight(FontWeight::SEMIBOLD).child("历史复盘"))
                        .children(journal.days.iter().rev().take(6).map(|(date, day)| {
                            let date = date.clone(); let current = date == self.date;
                            action(SharedString::from(format!("journal-history-{date}")), format!("{date}    {} 预案 · {} 草稿", day.plans.len(), day.drafts.len())).ghost().w_full()
                                .bg(rgb(if current { 0xe5f5f3 } else { 0xffffff })).text_color(rgb(if current { TEAL } else { INK }))
                                .on_click(cx.listener(move |this, _, window, cx| this.select_date(&date, window, cx)))
                        })))
                    .child(ui::panel().flex_1().min_h_0().flex().flex_col()
                        .child(div().p_3().flex().flex_col().gap_1().child(div().flex().items_center().child(div().flex_1().font_weight(FontWeight::SEMIBOLD).child("最近买入记录"))
                            .child(action("journal-add-buy", "＋").ghost().on_click(cx.listener(|this, _, window, cx| this.start_form(FormKind::Buy, None, window, cx)))))
                            .child(div().flex().items_center().child(ui::muted("最近 10 笔 · 按收益率展示"))
                                .child(div().flex_1()).child(action("journal-buy-history", "全部记录").ghost().text_color(rgb(TEAL)).on_click(cx.listener(|this, _, _, cx| { this.buys_history = true; cx.notify(); })))))
                        .child(div().id("journal-buys-scroll").flex_1().min_h_0().overflow_y_scroll().children(buys.iter().map(|buy| {
                            let edit_id = buy.id.clone(); let delete_id = buy.id.clone();
                            div().px_3().py_2().border_t_1().border_color(rgb(LINE)).flex().flex_col().gap_1()
                                .child(div().flex().items_center().gap_1().child(div().flex_1().font_weight(FontWeight::MEDIUM).child(buy.stock.clone()))
                                    .child(div().text_color(rgb(if buy.profit >= 0. { UP } else { DOWN })).font_weight(if buy.profit > 10. { FontWeight::BOLD } else { FontWeight::NORMAL }).child(format!("{:+.1}%", buy.profit))))
                                .child(ui::muted(format!("{} · {}", buy.date, buy.method)))
                                .child(div().flex().gap_1().child(action(SharedString::from(format!("journal-buy-edit-{}", buy.id)), "编辑").ghost().on_click(cx.listener(move |this, _, window, cx| this.start_form(FormKind::Buy, Some(edit_id.clone()), window, cx))))
                                    .child(action(SharedString::from(format!("journal-buy-delete-{}", buy.id)), "删除").ghost().text_color(rgb(MUTED)).on_click(cx.listener(move |this, _, _, cx| { this.delete = Some(DeleteTarget::Buy(delete_id.clone())); cx.notify(); }))))
                        })))
                        .child(div().p_3().border_t_1().border_color(rgb(LINE)).child(ui::muted("收益率为手工输入，不代表账户绩效。仅 >10% 加粗。"))))))
            .when_some(self.note_error.clone(), |el, error| el.child(div().id("journal-note-error").test_support().text_size(px(12.)).text_color(rgb(UP)).child(error)))
            .when(self.message.is_some() || (self.error.is_some() && !self.is_editing()), |el| el.child(div().text_size(px(12.)).text_color(rgb(if self.error.is_some() { UP } else { MUTED })).child(self.error.clone().or(self.message.clone()).unwrap_or_default())))
            .when(self.editor.is_some(), |el| el.child(self.editor_view(cx)))
            .when(self.buys_history, |el| el.child(self.buys_history_view(cx)))
            .when(self.methods_editor.is_some(), |el| el.child(self.methods_view(cx)))
            .when(self.form.is_some(), |el| el.child(self.form_view(cx)))
            .when(self.delete.is_some(), |el| el.child(self.delete_view(cx)))
    }
}

struct JournalReading {
    date: String,
    focus: FocusHandle,
    note: Entity<TextareaState>,
    note_synced: String,
    note_error: Option<String>,
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
        if self.persist_note(cx) {
            return true;
        }
        let detail = self.note_error.clone().unwrap_or_default();
        let key = format!("journal-note:{:?}", cx.entity_id());
        window.open_dialog(cx, move |dialog, _, _| {
            let key = key.clone();
            dialog.title("笔记修改尚未保存").width(px(460.)).child(
                div().flex().flex_col().gap_4().child(detail.clone())
                    .child("继续编辑可保留当前输入。放弃后只丢弃本窗口尚未保存的输入，已保存的笔记与预案保持不变。")
                    .child(div().flex().justify_end().gap_2()
                        .child(action("journal-reading-keep", "继续编辑").primary()
                            .on_click(|_, window, cx| window.close_dialog(cx)))
                        .child(action("journal-reading-discard", "放弃输入并关闭")
                            .on_click(move |_, window, cx| {
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

/// Exercise rejected input through production native entities and buttons. A
/// command rejection must not discard the only copy of a user's text.
#[cfg(feature = "test-support")]
pub(crate) fn smoke_rejected_inputs(cx: &mut HeadlessAppContext) -> anyhow::Result<()> {
    use anyhow::ensure;
    use gpui_kit::test::TestWindowExt;

    fn settle(cx: &mut HeadlessAppContext, handle: AnyWindowHandle) -> anyhow::Result<()> {
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| window.render_frame(cx))?;
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| window.render_frame(cx))?;
        Ok(())
    }
    let mut handles = Vec::new();
    let result = (|| -> anyhow::Result<()> {
        let (main, page) = cx.update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                        point(px(20.), px(20.)),
                        size(px(1536.), px(960.)),
                    ))),
                    ..Default::default()
                },
                cx,
                |window, cx| cx.new(|cx| JournalPage::new(window, cx)),
            )
        })?;
        handles.push(main);
        cx.update_window(main, |_, window, cx| {
            page.update(cx, |page, cx| page.create(window, cx))
        })?;
        let oversized_draft = "x".repeat(64 * 1024 + 1);
        cx.update_window(main, |_, window, cx| {
            let basis = page.read(cx).editor.as_ref().unwrap().basis.clone();
            basis.update(cx, |input, cx| {
                input.replace_all(oversized_draft.clone(), window, cx)
            });
        })?;
        settle(cx, main)?;
        for button in ["journal-draft-save", "journal-editor-close"] {
            cx.update_window(main, |_, window, cx| window.click(button, cx))?;
            settle(cx, main)?;
            cx.update(|cx| {
                let page = page.read(cx);
                ensure!(page.editor.is_some() && page.error.is_some());
                ensure!(
                    page.editor
                        .as_ref()
                        .unwrap()
                        .basis
                        .read(cx)
                        .value()
                        .as_ref()
                        == oversized_draft
                );
                ensure!(
                    !desktop_state::save_now(cx),
                    "Rejected draft did not protect application close"
                );
                Ok::<_, anyhow::Error>(())
            })?;
        }
        cx.update_window(main, |_, window, cx| {
            page.update(cx, |page, cx| page.select_date("2026-10-15", window, cx));
            ensure!(page.read(cx).date == TODAY && page.read(cx).editor.is_some());
            let basis = page.read(cx).editor.as_ref().unwrap().basis.clone();
            basis.update(cx, |input, cx| {
                input.replace_all("已修正的研究依据", window, cx)
            });
            Ok::<_, anyhow::Error>(())
        })??;
        settle(cx, main)?;
        cx.update_window(main, |_, window, cx| window.click("journal-draft-save", cx))?;
        settle(cx, main)?;
        ensure!(cx.update(|cx| page.read(cx).editor.is_none()));

        let oversized_note = "n".repeat(256 * 1024 + 1);
        cx.update_window(main, |_, window, cx| {
            let note = page.read(cx).note.clone();
            note.update(cx, |input, cx| {
                input.replace_all(oversized_note.clone(), window, cx)
            });
        })?;
        settle(cx, main)?;
        cx.update(|cx| {
            ensure!(page.read(cx).note_error.is_some());
            ensure!(page.read(cx).note.read(cx).value().as_ref() == oversized_note);
            desktop_state::dispatch(
                cx,
                Command::SetNote {
                    date: TODAY.into(),
                    text: "另一个窗口的有效笔记".into(),
                },
            )?;
            Ok::<_, anyhow::Error>(())
        })?;
        settle(cx, main)?;
        cx.update_window(main, |_, window, cx| {
            page.update(cx, |page, cx| page.select_date("2026-10-15", window, cx));
            ensure!(page.read(cx).date == TODAY);
            ensure!(page.read(cx).note.read(cx).value().as_ref() == oversized_note);
            let note = page.read(cx).note.clone();
            note.update(cx, |input, cx| {
                input.replace_all("修正后的笔记", window, cx)
            });
            Ok::<_, anyhow::Error>(())
        })??;
        settle(cx, main)?;
        ensure!(cx.update(desktop_state::save_now));

        let (reading, reader) = cx.update(|cx| {
            gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
                cx.new(|cx| JournalReading::new(TODAY.into(), window, cx))
            })
        })?;
        handles.push(reading);
        let (other, other_reader) = cx.update(|cx| {
            gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
                cx.new(|cx| JournalReading::new(TODAY.into(), window, cx))
            })
        })?;
        handles.push(other);
        cx.update_window(reading, |_, window, cx| {
            let note = reader.read(cx).note.clone();
            note.update(cx, |input, cx| {
                input.replace_all(oversized_note.clone(), window, cx)
            });
        })?;
        settle(cx, reading)?;
        cx.update_window(other, |_, window, cx| {
            let note = other_reader.read(cx).note.clone();
            note.update(cx, |input, cx| {
                input.replace_all("保留另一个阅读窗口的记录", window, cx)
            });
        })?;
        settle(cx, other)?;
        ensure!(
            !cx.update(desktop_state::save_now),
            "One reader cleared another reader's rejected input"
        );
        cx.update_window(reading, |_, window, cx| {
            ensure!(!reader.update(cx, |reader, cx| reader.request_close(window, cx)));
            Ok::<_, anyhow::Error>(())
        })??;
        settle(cx, reading)?;
        cx.update_window(reading, |_, window, cx| {
            window.click("journal-reading-keep", cx)
        })?;
        settle(cx, reading)?;
        ensure!(cx.update(|cx| reader.read(cx).note.read(cx).value().as_ref() == oversized_note));
        cx.update_window(reading, |_, window, cx| {
            ensure!(!reader.update(cx, |reader, cx| reader.request_close(window, cx)));
            Ok::<_, anyhow::Error>(())
        })??;
        settle(cx, reading)?;
        cx.update_window(reading, |_, window, cx| {
            window.click("journal-reading-discard", cx)
        })?;
        cx.run_until_parked();
        ensure!(cx.update(desktop_state::save_now));
        ensure!(
            cx.update(|cx| cx.global::<DesktopState>().journal.days[TODAY].note
                == "保留另一个阅读窗口的记录")
        );
        println!(
            "PASS: rejected drafts/notes retain native input, block destructive close/date changes, and recover without losing another reader's data"
        );
        Ok(())
    })();
    for handle in handles {
        let _ = cx.update_window(handle, |_, window, _| window.remove_window());
    }
    result
}
