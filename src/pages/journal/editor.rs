use super::*;

impl JournalPage {
    pub(super) fn create(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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

    pub(super) fn open_draft(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
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

    pub(super) fn edit_plan(&mut self, plan: Plan, window: &mut Window, cx: &mut Context<Self>) {
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

    pub(super) fn begin_editor(
        &mut self,
        draft: Draft,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
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

    pub(super) fn editor_draft(&self, cx: &App) -> Option<Draft> {
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

    pub(super) fn persist_editor(&mut self, cx: &mut Context<Self>) -> bool {
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

    pub(super) fn close_editor(
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

    pub(super) fn save_plan(&mut self, cx: &mut Context<Self>) {
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

    pub(super) fn toggle_symbol(&mut self, symbol: Symbol, cx: &mut Context<Self>) {
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
}
