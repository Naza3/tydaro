use super::*;

impl LimitPage {
    pub(super) fn pending_key(cx: &Context<Self>) -> String {
        format!("limit-editor-{:?}", cx.entity_id())
    }

    pub(super) fn report_pending(cx: &mut Context<Self>, error: Option<String>) {
        let key = Self::pending_key(cx);
        desktop_state::report_pending_input(cx, key, error);
    }

    pub(super) fn refresh_pending_input(&self, cx: &mut Context<Self>) {
        let pending = match &self.editing {
            Some(LimitEdit::Row(id)) => self.rows.iter().find(|r| &r.id == id).is_none_or(|row| {
                self.edit_name.read(cx).value().as_str() != row.name
                    || self.edit_board.read(cx).value().as_str() != row.board
                    || self.edit_reason.read(cx).value().as_str() != row.reason
            }),
            _ => false,
        };
        Self::report_pending(
            cx,
            pending.then(|| "涨停资料编辑尚未确认，请保存或取消。".into()),
        );
    }

    pub(super) fn cancel_edit(&mut self, cx: &mut Context<Self>) {
        self.editing = None;
        self.error = None;
        Self::report_pending(cx, None);
        cx.notify();
    }
    /// Commit a candidate copy; rejection never replaces the displayed source or editor inputs.
    pub(super) fn persist(&mut self, rows: Vec<ResearchRow>, cx: &mut Context<Self>) -> bool {
        if let Some(error) = self.load_error.clone() {
            self.error = Some(error.clone());
            cx.emit(DesktopEvent::Notice(error));
            return false;
        }
        match desktop_state::dispatch(cx, Command::ReplaceResearchRows(rows.clone())) {
            Ok(_) => {
                self.rows = rows;
                true
            }
            Err(error) => {
                let message = format!("研究修改未应用：{error}");
                self.error = Some(message.clone());
                if matches!(self.editing, Some(LimitEdit::Row(_))) {
                    Self::report_pending(cx, Some(message.clone()));
                }
                cx.emit(DesktopEvent::Notice(message));
                cx.notify();
                false
            }
        }
    }

    pub(super) fn undo_delete(&mut self, cx: &mut Context<Self>) {
        if self.editing.is_some() {
            return;
        }
        let mut rows = self.rows.clone();
        restore_rows(&mut rows, self.deleted.clone());
        if self.persist(rows, cx) {
            if let Some(row) = self.deleted.first() {
                self.selected_date = row.date.clone();
                self.selected_sector = row.sector.clone();
            }
            self.deleted.clear();
            self.error = None;
            cx.emit(DesktopEvent::Notice("已恢复整日资料".into()));
        }
        cx.notify();
    }

    pub(super) fn begin_edit(
        &mut self,
        row: ResearchRow,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.load_error.is_some() {
            return;
        }
        if self.editing.is_some() {
            cx.emit(DesktopEvent::Notice("请先保存或取消当前资料编辑".into()));
            return;
        }
        self.edit_name
            .update(cx, |input, cx| input.set_value(row.name, window, cx));
        self.edit_board
            .update(cx, |input, cx| input.set_value(row.board, window, cx));
        self.edit_reason
            .update(cx, |input, cx| input.set_value(row.reason, window, cx));
        self.editing = Some(LimitEdit::Row(row.id));
        self.error = None;
        self.edit_name
            .update(cx, |input, cx| input.focus(window, cx));
        cx.notify();
    }

    pub(super) fn confirm_edit(&mut self, cx: &mut Context<Self>) {
        if self.load_error.is_some() {
            return;
        }
        let mut rows = self.rows.clone();
        let mut selection = None;
        let mut deleted = None;
        let mut imported = false;
        match self.editing.as_ref() {
            Some(LimitEdit::Import) => {
                if !rows.iter().any(|r| r.date == "2026-10-15") {
                    let mut incoming = sample_rows("2026-10-15", 0);
                    incoming.append(&mut rows);
                    rows = incoming;
                    selection = Some(("2026-10-15".to_owned(), SECTORS[0].to_owned()));
                    imported = true;
                }
            }
            Some(LimitEdit::Delete(date)) => {
                deleted = Some(rows.iter().filter(|r| &r.date == date).cloned().collect());
                rows.retain(|r| &r.date != date);
                if let Some(row) = rows.first() {
                    selection = Some((row.date.clone(), row.sector.clone()));
                }
            }
            Some(LimitEdit::Row(id)) => {
                let name = self.edit_name.read(cx).value().trim().to_owned();
                let board = self.edit_board.read(cx).value().trim().to_owned();
                if name.is_empty() || board.is_empty() {
                    let message = "请填写名称和连板字段；空白原因可以保留。".to_owned();
                    self.error = Some(message.clone());
                    Self::report_pending(cx, Some(message));
                    cx.notify();
                    return;
                }
                let Some(row) = rows.iter_mut().find(|r| &r.id == id) else {
                    let message = "原研究资料已不存在，请保留输入后取消编辑。".to_owned();
                    self.error = Some(message.clone());
                    Self::report_pending(cx, Some(message));
                    cx.notify();
                    return;
                };
                row.name = name;
                row.board = board;
                row.reason = self.edit_reason.read(cx).value().to_string();
            }
            None => return,
        }
        if !self.persist(rows, cx) {
            return;
        }
        if let Some((date, sector)) = selection {
            self.selected_date = date;
            self.selected_sector = sector;
        }
        if let Some(deleted) = deleted {
            self.deleted = deleted;
        }
        if imported {
            self.search_tab = false;
        }
        self.editing = None;
        self.error = None;
        Self::report_pending(cx, None);
        cx.emit(DesktopEvent::Notice(
            "涨停资料已修改；题材页独立示例不会自动重算".into(),
        ));
        cx.notify();
    }

    pub(super) fn editor(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(edit) = &self.editing else {
            return div().into_any_element();
        };
        let (title, description) = match edit {
            LimitEdit::Import => (
                "导入示例资料",
                if self.rows.iter().any(|row| row.date == "2026-10-15") {
                    "2026-10-15 的示例资料已导入，无需重复添加。关闭此面板可继续查看或编辑。"
                        .to_owned()
                } else {
                    "载入 2026-10-15 的内置样例，用于体验资料流程。当前不读取真实文件，不执行旧格式兼容或算法重算。".to_owned()
                },
            ),
            LimitEdit::Delete(date) => (
                "删除整日资料",
                format!(
                    "将删除 {date} 的 {} 条资料。删除后可在本页撤销最近一次整日删除。",
                    self.rows.iter().filter(|r| &r.date == date).count()
                ),
            ),
            LimitEdit::Row(_) => (
                "编辑资料",
                "只修改此条导入记录，不修改股票目录或历史题材样例。".to_owned(),
            ),
        };
        panel()
            .id("limit-editor")
            .border_color(rgb(TEAL))
            .p_4()
            .flex()
            .flex_col()
            .gap_3()
            .child(div().font_weight(FontWeight::SEMIBOLD).child(title))
            .child(muted(description))
            .when(matches!(edit, LimitEdit::Row(_)), |view| {
                view.child(
                    div()
                        .flex()
                        .gap_3()
                        .child(
                            div()
                                .flex_1()
                                .child(muted("股票名称"))
                                .child(Input::new(&self.edit_name).id("limit-edit-name")),
                        )
                        .child(
                            div()
                                .flex_1()
                                .child(muted("连板字段"))
                                .child(Input::new(&self.edit_board).id("limit-edit-board")),
                        ),
                )
                .child(
                    div()
                        .id("limit-edit-reason")
                        .test_support()
                        .child(Textarea::new(&self.edit_reason).h(px(96.))),
                )
            })
            .when_some(self.error.clone(), |view, error| {
                view.child(div().text_color(rgb(0xc92f57)).child(error))
            })
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap_2()
                    .child(button("limit-edit-cancel", "取消").on_click(cx.listener(
                        |this, _, _, cx| {
                            this.cancel_edit(cx);
                        },
                    )))
                    .child(
                        button("limit-edit-confirm", "确认")
                            .disabled(
                                matches!(edit, LimitEdit::Import)
                                    && self.rows.iter().any(|row| row.date == "2026-10-15"),
                            )
                            .primary()
                            .on_click(cx.listener(|this, _, _, cx| this.confirm_edit(cx))),
                    ),
            )
            .into_any_element()
    }
}
