//! Window-local drafts and guarded asynchronous text-file reads.
use super::*;

impl PanelContent {
    pub(in crate::panels) fn refresh_import(&mut self, cx: &App) {
        self.import_preview =
            stock_import::parse(&self.import_text().read(cx).value(), &self.members);
    }

    pub(in crate::panels) fn leave_stock_import(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.stock_list_page = StockListPage::List;
        self.import_request = self.import_request.wrapping_add(1);
        self.import_loading = false;
        self.import_message = None;
        self.import_preview = stock_import::parse("", &self.members);
        self.import_text()
            .update(cx, |input, cx| input.set_value("", window, cx));
    }

    pub(in crate::panels) fn show_stock_import(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.leave_stock_import(window, cx);
        self.stock_list_page = StockListPage::Import;
        self.import_text()
            .update(cx, |input, cx| input.focus(window, cx));
        cx.notify();
    }

    pub(in crate::panels) fn apply_stock_import(&mut self, cx: &mut Context<Self>) {
        if self.import_loading
            || self
                .import_message
                .as_ref()
                .is_some_and(|(_, error)| *error)
        {
            return;
        }
        // Resolve against the current draft and membership again at commit time.
        self.refresh_import(cx);
        let Ok(preview) = &self.import_preview else {
            return;
        };
        if preview.additions.is_empty() {
            return;
        }
        let count = preview.additions.len();
        self.members.extend(preview.additions.iter().copied());
        self.refresh_import(cx);
        self.import_message = Some((format!("已添加 {count} 支股票到当前列表"), false));
        cx.emit(ContentEvent::ListChanged);
        cx.notify();
    }

    pub(in crate::panels) fn read_import_file(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.import_loading {
            return;
        }
        self.import_request = self.import_request.wrapping_add(1);
        let request = self.import_request;
        let revision = self.import_revision;
        self.import_loading = true;
        self.import_message = None;
        let selected = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("选择股票文本文件".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = match selected.await {
                Ok(Ok(Some(paths))) => {
                    if let Some(path) = paths.into_iter().next() {
                        cx.background_executor()
                            .spawn(async move {
                                use std::io::Read as _;
                                let file = std::fs::File::open(&path)
                                    .map_err(|error| format!("无法读取文件：{error}"))?;
                                let metadata =
                                    file.metadata().map_err(|error| error.to_string())?;
                                if !metadata.is_file() {
                                    return Err("请选择普通文本文件".into());
                                }
                                if metadata.len() > stock_import::MAX_INPUT_BYTES as u64 {
                                    return Err("文件超过 256 KiB，请拆分后导入".into());
                                }
                                let mut bytes = Vec::new();
                                file.take(stock_import::MAX_INPUT_BYTES as u64 + 1)
                                    .read_to_end(&mut bytes)
                                    .map_err(|error| format!("无法读取文件：{error}"))?;
                                stock_import::decode_text(&bytes)
                                    .map(Some)
                                    .map_err(|error| error.to_string())
                            })
                            .await
                    } else {
                        Ok(None)
                    }
                }
                Ok(Ok(None)) => Ok(None),
                Ok(Err(error)) => Err(format!("无法打开文件选择器：{error}")),
                Err(_) => Err("文件选择未完成，请重试或粘贴文本".into()),
            };
            let _ = this.update_in(cx, |this, window, cx| {
                if this.import_request != request || this.stock_list_page != StockListPage::Import {
                    return;
                }
                this.import_loading = false;
                if this.import_revision != revision {
                    this.import_message = Some((
                        "文本已修改，已保留当前输入；如需文件内容请重新读取".into(),
                        false,
                    ));
                } else {
                    match result {
                        Ok(Some(text)) => {
                            this.import_text()
                                .update(cx, |input, cx| input.set_value(text, window, cx));
                            this.refresh_import(cx);
                        }
                        Ok(None) => {}
                        Err(error) => this.import_message = Some((error, true)),
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub(in crate::panels) fn stock_import_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let preview = self.import_preview.as_ref().ok();
        let count = preview.map_or(0, |p| p.additions.len());
        let summary = preview.map_or_else(
            || "识别未完成".to_string(),
            |p| {
                format!(
                    "可添加 {} 支 · 已有 {} 支 · 重复 {} 次 · 未收录 {} 项",
                    p.additions.len(),
                    p.already_present,
                    p.duplicates,
                    p.unknown.len()
                )
            },
        );
        let blocked = self.import_loading
            || self
                .import_message
                .as_ref()
                .is_some_and(|(_, error)| *error);
        div()
            .size_full()
            .flex()
            .flex_col()
            .min_h_0()
            .child(
                div()
                    .px_2()
                    .py_1()
                    .flex()
                    .items_center()
                    .justify_between()
                    .flex_shrink_0()
                    .child(div().font_weight(FontWeight::MEDIUM).child("从文本导入"))
                    .child(
                        Button::new("stock-import-cancel")
                            .label("返回列表")
                            .small()
                            .ghost()
                            .text_color(rgb(TEAL))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.leave_stock_import(window, cx);
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .id("stock-import-body")
                    .test_support()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(
                        div()
                            .px_2()
                            .pb_1()
                            .text_size(px(12.))
                            .text_color(rgb(MUTED))
                            .flex_shrink_0()
                            .child("粘贴代码、名称或表格；支持换行、空格及中英文逗号。"),
                    )
                    .child(
                        div()
                            .id("stock-import-text")
                            .test_support()
                            .px_2()
                            .flex_shrink_0()
                            .child(
                                Textarea::new(self.import_text())
                                    .accessibility_id("stock-import-text-input")
                                    .aria_label("待导入股票文本")
                                    .small()
                                    .h(px(106.))
                                    .w_full(),
                            ),
                    )
                    .child(
                        div()
                            .px_2()
                            .py_1()
                            .flex()
                            .items_center()
                            .gap_1()
                            .flex_shrink_0()
                            .child(
                                Button::new("stock-import-file")
                                    .label(if self.import_loading {
                                        "读取中…"
                                    } else {
                                        "读取文件"
                                    })
                                    .small()
                                    .ghost()
                                    .disabled(self.import_loading)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.read_import_file(window, cx)
                                    })),
                            )
                            .child(
                                Button::new("stock-import-clear")
                                    .label("清空")
                                    .small()
                                    .ghost()
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.import_request = this.import_request.wrapping_add(1);
                                        this.import_loading = false;
                                        this.import_message = None;
                                        this.import_text().update(cx, |input, cx| {
                                            input.set_value("", window, cx)
                                        });
                                        this.refresh_import(cx);
                                        cx.notify();
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .px_2()
                            .pb_1()
                            .text_size(px(12.))
                            .text_color(rgb(MUTED))
                            .flex_shrink_0()
                            .child("TXT / CSV · UTF-8、UTF-16、GB18030 · 最多 256 KiB"),
                    )
                    .child(
                        div()
                            .id("stock-import-summary")
                            .test_support()
                            .aria_label(summary.clone())
                            .px_2()
                            .py_1()
                            .bg(rgb(0xe8f6f6))
                            .text_size(px(12.))
                            .flex_shrink_0()
                            .child(summary),
                    )
                    .when_some(self.import_preview.as_ref().err(), |view, error| {
                        view.child(
                            div()
                                .px_2()
                                .py_1()
                                .text_color(rgb(RED))
                                .child(error.to_string()),
                        )
                    })
                    .when_some(self.import_message.as_ref(), |view, (message, error)| {
                        view.child(
                            div()
                                .id("stock-import-message")
                                .test_support()
                                .aria_label(message.clone())
                                .px_2()
                                .py_1()
                                .text_color(rgb(if *error { RED } else { TEAL }))
                                .child(message.clone()),
                        )
                    })
                    .child(
                        div()
                            .id("stock-import-preview")
                            .test_support()
                            .children(preview.into_iter().flat_map(|p| p.recognized.iter()).map(
                                |symbol| {
                                    let exists = self.members.contains(symbol);
                                    div()
                                        .px_2()
                                        .py_2()
                                        .flex()
                                        .gap_2()
                                        .items_center()
                                        .border_b_1()
                                        .border_color(rgb(LINE))
                                        .child(div().flex_1().min_w_0().child(format!(
                                            "{}  {}",
                                            symbol.code(),
                                            symbol.name()
                                        )))
                                        .child(
                                            div()
                                                .text_size(px(12.))
                                                .text_color(rgb(if exists { MUTED } else { TEAL }))
                                                .flex_shrink_0()
                                                .child(if exists {
                                                    "已在列表"
                                                } else {
                                                    "待添加"
                                                }),
                                        )
                                },
                            ))
                            .children(
                                preview
                                    .into_iter()
                                    .flat_map(|p| p.unknown.iter().take(200))
                                    .map(|code| {
                                        div()
                                            .px_2()
                                            .py_2()
                                            .flex()
                                            .gap_2()
                                            .items_center()
                                            .border_b_1()
                                            .border_color(rgb(LINE))
                                            .child(div().flex_1().min_w_0().child(code.clone()))
                                            .child(
                                                div()
                                                    .text_size(px(12.))
                                                    .text_color(rgb(MUTED))
                                                    .flex_shrink_0()
                                                    .child("目录未收录"),
                                            )
                                    }),
                            )
                            .when(preview.is_some_and(|p| p.unknown.len() > 200), |view| {
                                view.child(
                                    div()
                                        .p_2()
                                        .text_color(rgb(MUTED))
                                        .child("未收录项目仅预览前 200 项"),
                                )
                            })
                            .when(
                                preview.is_some_and(|p| {
                                    p.recognized.is_empty() && p.unknown.is_empty()
                                }),
                                |view| {
                                    view.child(
                                        div()
                                            .p_4()
                                            .text_color(rgb(MUTED))
                                            .child("输入后自动识别，确认添加前不改变当前列表"),
                                    )
                                },
                            ),
                    ),
            )
            .child(
                div()
                    .p_2()
                    .border_t_1()
                    .border_color(rgb(LINE))
                    .flex_shrink_0()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        Button::new("stock-import-confirm")
                            .label(format!("添加 {count} 支到当前列表"))
                            .small()
                            .primary()
                            .w_full()
                            .disabled(count == 0 || blocked)
                            .on_click(cx.listener(|this, _, _, cx| this.apply_stock_import(cx))),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(rgb(MUTED))
                            .child("仅匹配当前模拟目录，自动跳过已有和重复股票"),
                    ),
            )
            .into_any_element()
    }
}
