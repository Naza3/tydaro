//! Compact board navigation and reversible, inline board management.
use crate::workspace::{self, BoardPreset};
use gpui_kit::{
    component::{
        Disableable, Sizable,
        button::{Button, ButtonVariants},
        input::{Input, InputEvent, InputState},
        menu::{DropdownMenu, PopupMenuItem},
    },
    prelude::*,
    *,
};

const INK: u32 = 0x233348;
const MUTED: u32 = 0x85909f;
const TEAL: u32 = 0x009b9a;
const RED: u32 = 0xef4770;

#[derive(Clone, Copy)]
enum Edit {
    Create(BoardPreset),
    Duplicate,
    Rename(u64),
    Delete(u64),
}

pub(crate) struct BoardBar {
    input: Entity<InputState>,
    editing: Option<Edit>,
    error: Option<String>,
    scroll: ScrollHandle,
    selected: Option<u64>,
    _subscription: Subscription,
}

impl BoardBar {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("看板名称"));
        let subscription = cx.subscribe(&input, |this, _, event, cx| match event {
            InputEvent::PressEnter { .. } => this.confirm(cx),
            InputEvent::Change => {
                this.error = None;
                cx.notify();
            }
            _ => {}
        });
        Self {
            input,
            editing: None,
            error: None,
            scroll: ScrollHandle::new(),
            selected: None,
            _subscription: subscription,
        }
    }

    fn begin(&mut self, edit: Edit, window: &mut Window, cx: &mut Context<Self>) {
        let boards = workspace::board_summaries(cx);
        let current = workspace::active_board_id(cx);
        let current_name = boards
            .iter()
            .find(|(id, _)| *id == current)
            .map(|(_, name)| name.as_str())
            .unwrap_or("看板");
        let name = match edit {
            Edit::Create(BoardPreset::Empty) => available_name("新看板", &boards),
            Edit::Create(BoardPreset::Standard) => available_name("默认行情", &boards),
            Edit::Create(BoardPreset::DualChart) => available_name("双图对比", &boards),
            Edit::Duplicate => {
                let base: String = current_name.chars().take(20).collect();
                available_name(&format!("{base} 副本"), &boards)
            }
            Edit::Rename(id) | Edit::Delete(id) => boards
                .iter()
                .find(|(board, _)| *board == id)
                .map(|(_, name)| name.clone())
                .unwrap_or_default(),
        };
        self.editing = Some(edit);
        self.error = None;
        self.input.update(cx, |input, cx| {
            input.set_value(name, window, cx);
            if !matches!(edit, Edit::Delete(_)) {
                input.focus(window, cx);
                input.select_all(window, cx);
            }
        });
        cx.notify();
    }

    fn cancel(&mut self, cx: &mut Context<Self>) {
        self.editing = None;
        self.error = None;
        cx.notify();
    }

    fn confirm(&mut self, cx: &mut Context<Self>) {
        let Some(edit) = self.editing else { return };
        let name = self.input.read(cx).value().to_string();
        let owner = cx.weak_entity();
        // Board switching can create and close native windows. Run after the
        // current input/window callback has released its borrow.
        cx.defer(move |cx| {
            let result = match edit {
                Edit::Create(preset) => workspace::create_board(&name, preset, cx).map(|_| ()),
                Edit::Duplicate => workspace::duplicate_board(&name, cx).map(|_| ()),
                Edit::Rename(id) => workspace::rename_board(id, &name, cx),
                Edit::Delete(id) => workspace::delete_board(id, cx),
            };
            let _ = owner.update(cx, |this, cx| {
                match result {
                    Ok(()) => {
                        this.editing = None;
                        this.error = None;
                    }
                    Err(error) => this.error = Some(error.to_string()),
                }
                cx.notify();
            });
        });
    }

    fn start_edit(owner: WeakEntity<Self>, edit: Edit, window: &mut Window, cx: &mut App) {
        // Let the menu finish restoring its trigger focus before focusing the
        // inline name field; the first keystroke should replace its selection.
        window.defer(cx, move |window, cx| {
            let _ = owner.update(cx, |this, cx| this.begin(edit, window, cx));
        });
    }

    fn switch(owner: WeakEntity<Self>, id: u64, cx: &mut App) {
        cx.defer(move |cx| {
            let result = workspace::switch_board(id, cx);
            let _ = owner.update(cx, |this, cx| {
                this.error = result.err().map(|error| error.to_string());
                cx.notify();
            });
        });
    }

    fn reorder(owner: WeakEntity<Self>, id: u64, offset: isize, cx: &mut App) {
        cx.defer(move |cx| {
            let result = workspace::move_board(id, offset, cx);
            let _ = owner.update(cx, |this, cx| {
                this.error = result.err().map(|error| error.to_string());
                this.selected = None;
                cx.notify();
            });
        });
    }

    fn editor(&self, edit: Edit, cx: &mut Context<Self>) -> AnyElement {
        let deleting = matches!(edit, Edit::Delete(_));
        let mut row = div().h(px(28.)).flex().items_center().gap_1().min_w_0();
        if deleting {
            row = row.child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_color(rgb(INK))
                    .child(format!(
                        "删除“{}”及其组件布局？",
                        self.input.read(cx).value()
                    )),
            );
        } else {
            row = row
                .child(
                    div()
                        .flex_shrink_0()
                        .text_color(rgb(MUTED))
                        .child(match edit {
                            Edit::Create(_) => "新建看板",
                            Edit::Duplicate => "复制看板",
                            Edit::Rename(_) => "重命名",
                            Edit::Delete(_) => unreachable!(),
                        }),
                )
                .child(
                    div().flex_1().min_w_0().child(
                        Input::new(&self.input)
                            .id("board-name-input")
                            .small()
                            .w_full(),
                    ),
                );
        }
        row.child(
            action(
                if deleting {
                    "board-delete-confirm"
                } else {
                    "board-name-confirm"
                },
                if deleting { "确认删除" } else { "确认" },
            )
            .text_color(rgb(if deleting { RED } else { TEAL }))
            .on_click(cx.listener(|this, _, _, cx| this.confirm(cx))),
        )
        .child(
            action("board-name-cancel", "取消")
                .on_click(cx.listener(|this, _, _, cx| this.cancel(cx))),
        )
        .into_any_element()
    }
}

impl Render for BoardBar {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let boards = workspace::board_summaries(cx);
        let active = workspace::active_board_id(cx);
        let index = boards.iter().position(|(id, _)| *id == active).unwrap_or(0);
        if self.selected != Some(active) {
            self.scroll.scroll_to_item(index);
            self.selected = Some(active);
        }
        let owner = cx.weak_entity();
        let full = boards.len() >= 16;
        let only = boards.len() <= 1;
        let at_start = index == 0;
        let at_end = index + 1 >= boards.len();
        let row = if let Some(edit) = self.editing {
            self.editor(edit, cx)
        } else {
            let add_owner = owner.clone();
            let manage_owner = owner.clone();
            div()
                .h(px(28.))
                .flex()
                .items_center()
                .min_w_0()
                .gap_1()
                .child(
                    div()
                        .id("board-tabs")
                        .flex_1()
                        .min_w_0()
                        .h_full()
                        .flex()
                        .items_center()
                        .overflow_x_scroll()
                        .track_scroll(&self.scroll)
                        .children(boards.iter().map(|(id, name)| {
                            let id = *id;
                            let owner = owner.clone();
                            div()
                                .id(SharedString::from(format!("board-tab-{id}")))
                                .h_full()
                                .px_3()
                                .max_w(px(164.))
                                .flex_shrink_0()
                                .flex()
                                .items_center()
                                .border_b_2()
                                .border_color(rgb(if id == active { TEAL } else { 0xffffff }))
                                .bg(rgb(if id == active { 0xe8f7f6 } else { 0xffffff }))
                                .text_color(rgb(if id == active { TEAL } else { INK }))
                                .cursor_pointer()
                                .hover(|style| style.bg(rgb(0xeef4f7)))
                                .child(div().truncate().child(name.clone()))
                                .on_click(move |_, _, cx| Self::switch(owner.clone(), id, cx))
                                .test_support()
                        })),
                )
                .child(
                    Button::new("board-add")
                        .label("＋")
                        .tooltip("新建看板")
                        .small()
                        .ghost()
                        .disabled(full)
                        .dropdown_menu(move |mut menu, _, _| {
                            for (label, preset) in [
                                ("空白看板", BoardPreset::Empty),
                                ("默认行情看板", BoardPreset::Standard),
                                ("双图对比看板", BoardPreset::DualChart),
                            ] {
                                let owner = add_owner.clone();
                                menu = menu.item(PopupMenuItem::new(label).on_click(
                                    move |_, window, cx| {
                                        Self::start_edit(
                                            owner.clone(),
                                            Edit::Create(preset),
                                            window,
                                            cx,
                                        )
                                    },
                                ));
                            }
                            menu
                        }),
                )
                .child(
                    Button::new("board-manage")
                        .label("看板 ▾")
                        .small()
                        .ghost()
                        .dropdown_menu(move |menu, window, cx| {
                            let duplicate_owner = manage_owner.clone();
                            let rename_owner = manage_owner.clone();
                            let delete_owner = manage_owner.clone();
                            let left_owner = manage_owner.clone();
                            let right_owner = manage_owner.clone();
                            let all_owner = manage_owner.clone();
                            menu.item(PopupMenuItem::new("复制当前看板").disabled(full).on_click(
                                move |_, window, cx| {
                                    Self::start_edit(
                                        duplicate_owner.clone(),
                                        Edit::Duplicate,
                                        window,
                                        cx,
                                    )
                                },
                            ))
                            .item(PopupMenuItem::new("重命名当前看板").on_click(
                                move |_, window, cx| {
                                    Self::start_edit(
                                        rename_owner.clone(),
                                        Edit::Rename(active),
                                        window,
                                        cx,
                                    )
                                },
                            ))
                            .separator()
                            .item(PopupMenuItem::new("向左移动").disabled(at_start).on_click(
                                move |_, _, cx| Self::reorder(left_owner.clone(), active, -1, cx),
                            ))
                            .item(PopupMenuItem::new("向右移动").disabled(at_end).on_click(
                                move |_, _, cx| Self::reorder(right_owner.clone(), active, 1, cx),
                            ))
                            .item(PopupMenuItem::new("删除当前看板").disabled(only).on_click(
                                move |_, window, cx| {
                                    Self::start_edit(
                                        delete_owner.clone(),
                                        Edit::Delete(active),
                                        window,
                                        cx,
                                    )
                                },
                            ))
                            .separator()
                            .submenu(
                                "全部看板",
                                window,
                                cx,
                                move |mut menu, _, cx| {
                                    for (id, name) in workspace::board_summaries(cx) {
                                        let owner = all_owner.clone();
                                        menu = menu.item(
                                            PopupMenuItem::new(name)
                                                .checked(id == active)
                                                .on_click(move |_, _, cx| {
                                                    Self::switch(owner.clone(), id, cx)
                                                }),
                                        );
                                    }
                                    menu
                                },
                            )
                        }),
                )
                .into_any_element()
        };
        div()
            .id("board-bar")
            .flex_1()
            .min_w_0()
            .h(px(40.))
            .flex()
            .flex_col()
            .justify_center()
            .text_size(px(12.))
            .capture_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                if this.editing.is_some() && event.keystroke.key == "escape" {
                    this.cancel(cx);
                    cx.stop_propagation();
                }
            }))
            .child(row)
            .when_some(self.error.clone(), |this, error| {
                this.child(
                    div()
                        .h(px(12.))
                        .min_w_0()
                        .truncate()
                        .text_size(px(10.))
                        .text_color(rgb(RED))
                        .child(error),
                )
            })
    }
}

fn action(id: &'static str, label: &'static str) -> Button {
    Button::new(id)
        .label(label)
        .small()
        .ghost()
        .flex_shrink_0()
        .text_color(rgb(INK))
}

fn available_name(base: &str, boards: &[(u64, String)]) -> String {
    let base: String = base.chars().take(24).collect();
    if !boards.iter().any(|(_, name)| name == &base) {
        return base;
    }
    for index in 2.. {
        let suffix = format!(" {index}");
        let prefix: String = base.chars().take(24 - suffix.chars().count()).collect();
        let name = format!("{prefix}{suffix}");
        if !boards.iter().any(|(_, existing)| existing == &name) {
            return name;
        }
    }
    unreachable!()
}

#[cfg(test)]
mod tests {
    use super::available_name;

    #[test]
    fn long_duplicate_names_stay_valid_and_unique() {
        let base = format!("{} 副本", "盘".repeat(20));
        let mut boards = vec![(1, base.clone())];
        for id in 2..=16 {
            let name = available_name(&base, &boards);
            assert_eq!(crate::boards::validate_board_name(&name).unwrap(), name);
            assert!(!boards.iter().any(|(_, existing)| existing == &name));
            boards.push((id, name));
        }
        let name = available_name(&"盘".repeat(30), &[]);
        assert_eq!(name.chars().count(), 24);
        assert!(crate::boards::validate_board_name(&name).is_ok());
    }
}
