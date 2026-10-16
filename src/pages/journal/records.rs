use super::*;

impl JournalPage {
    pub(super) fn start_form(
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

    pub(super) fn save_form(&mut self, cx: &mut Context<Self>) {
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

    pub(super) fn confirm_delete(&mut self, cx: &mut Context<Self>) {
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

    pub(super) fn open_methods(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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

    pub(super) fn save_methods(&mut self, cx: &mut Context<Self>) {
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
}
