//! In-app native foundation comparison, deliberately outside desktop persistence.
use crate::{
    gallery_controls::ControlSamples,
    gallery_display::DisplaySamples,
    gallery_preferences::{self, GalleryPreferences},
    gallery_selection::SelectionSamples,
    ui_foundation::{self as ui, Family, Variant},
};
use gpui_kit::{
    component::{
        Selectable,
        button::{Button, ButtonVariants},
    },
    prelude::*,
    *,
};

pub struct ComponentGallery {
    family: Family,
    solo: Option<Variant>,
    copied: bool,
    focus: FocusHandle,
    scroll: ScrollHandle,
    controls: Vec<Entity<ControlSamples>>,
    selection: Vec<Entity<SelectionSamples>>,
    display: Vec<Entity<DisplaySamples>>,
}
impl ComponentGallery {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            family: Family::Actions,
            solo: None,
            copied: false,
            focus: cx.focus_handle(),
            scroll: ScrollHandle::new(),
            controls: Variant::ALL
                .map(|variant| cx.new(|cx| ControlSamples::new(variant, window, cx)))
                .into(),
            selection: Variant::ALL
                .map(|variant| cx.new(|cx| SelectionSamples::new(variant, window, cx)))
                .into(),
            display: Variant::ALL
                .map(|variant| cx.new(|cx| DisplaySamples::new(variant, window, cx)))
                .into(),
        }
    }
    fn select_family(&mut self, family: Family, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus, cx);
        self.scroll.set_offset(point(px(0.), px(0.)));
        self.family = family;
        for samples in &self.controls {
            samples.update(cx, |samples, cx| samples.set_family(family, cx));
        }
        for samples in &self.selection {
            samples.update(cx, |samples, cx| samples.set_family(family, cx));
        }
        for samples in &self.display {
            samples.update(cx, |samples, cx| samples.set_family(family, cx));
        }
        cx.notify();
    }
    fn sample(&self, variant: Variant) -> AnyElement {
        let index = variant.index();
        match self.family {
            Family::Actions | Family::Input | Family::Number => {
                self.controls[index].clone().into_any_element()
            }
            Family::Choice | Family::Toggle | Family::Tabs | Family::Tokens => {
                self.selection[index].clone().into_any_element()
            }
            Family::List | Family::Disclosure | Family::Feedback => {
                self.display[index].clone().into_any_element()
            }
        }
    }
    fn column(&self, variant: Variant, cx: &mut Context<Self>) -> AnyElement {
        let chosen =
            cx.global::<GalleryPreferences>().choices[self.family.index()] == Some(variant);
        div()
            .flex_1()
            .min_w_0()
            .max_w(px(if self.solo.is_some() { 760. } else { 2000. }))
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .flex()
                    .items_start()
                    .gap_3()
                    .child(
                        div()
                            .size(px(34.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .border_1()
                            .border_color(rgb(ui::LINE))
                            .rounded_md()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(ui::ACCENT))
                            .child(variant.letter()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .child(ui::caption(variant.title()))
                            .child(ui::hint(variant.description())),
                    ),
            )
            .child(
                div()
                    .min_h(px(370.))
                    .p_4()
                    .bg(rgb(ui::SURFACE))
                    .border_1()
                    .border_color(rgb(ui::LINE))
                    .rounded_lg()
                    .child(self.sample(variant)),
            )
            .child(
                div()
                    .min_h(px(65.))
                    .child(ui::hint(self.family.differences()[variant.index()])),
            )
            .child(
                Button::new(format!("gallery-prefer-{}", variant.key()))
                    .w_full()
                    .h(px(34.))
                    .selected(chosen)
                    .label(if chosen {
                        format!("已选 {} · 点击撤回", variant.letter())
                    } else {
                        format!("这一类倾向 {}", variant.letter())
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.copied = false;
                        gallery_preferences::choose(this.family, variant, cx);
                    })),
            )
            .into_any_element()
    }
    #[cfg(feature = "test-support")]
    pub(crate) fn selected_family(&self) -> Family {
        self.family
    }
    #[cfg(feature = "test-support")]
    pub(crate) fn preference(&self, family: Family, cx: &App) -> Option<Variant> {
        cx.global::<GalleryPreferences>().choices[family.index()]
    }
    #[cfg(feature = "test-support")]
    pub(crate) fn controls_snapshot(
        &self,
        variant: Variant,
        cx: &App,
    ) -> (String, f64, bool, bool) {
        self.controls[variant.index()].read(cx).snapshot(cx)
    }
    #[cfg(feature = "test-support")]
    pub(crate) fn controls_focus(
        &self,
        variant: Variant,
        window: &Window,
        cx: &App,
    ) -> (bool, bool, bool) {
        self.controls[variant.index()]
            .read(cx)
            .focus_snapshot(window, cx)
    }
    #[cfg(feature = "test-support")]
    pub(crate) fn selection_snapshot(
        &self,
        variant: Variant,
        cx: &App,
    ) -> (usize, bool, usize, [bool; 3]) {
        self.selection[variant.index()].read(cx).snapshot()
    }
    #[cfg(feature = "test-support")]
    pub(crate) fn display_snapshot(&self, variant: Variant, cx: &App) -> (usize, bool, usize) {
        self.display[variant.index()].read(cx).snapshot()
    }
}
impl ComponentGallery {
    // Keep each builder tree behind a type-erased boundary. In debug builds,
    // GPUI's by-value styles otherwise leave many large temporaries on one stack.
    fn family_button(&self, item: Family, cx: &mut Context<Self>) -> AnyElement {
        let label = div()
            .flex()
            .w_full()
            .items_center()
            .gap_2()
            .child(
                div()
                    .w(px(20.))
                    .text_size(px(11.))
                    .text_color(rgb(ui::MUTED))
                    .child(format!("{:02}", item.index() + 1)),
            )
            .child(div().flex_1().text_size(px(12.)).child(item.label()))
            .when_some(
                cx.global::<GalleryPreferences>().choices[item.index()],
                |this, variant| {
                    this.child(
                        div()
                            .text_color(rgb(ui::ACCENT))
                            .text_size(px(11.))
                            .child(variant.letter()),
                    )
                },
            )
            .into_any_element();
        Button::new(format!("gallery-family-{}", item.key()))
            .ghost()
            .w_full()
            .h(px(38.))
            .selected(item == self.family)
            .child(label)
            .on_click(cx.listener(move |this, _, window, cx| this.select_family(item, window, cx)))
            .into_any_element()
    }

    fn sidebar(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .w(px(178.))
            .h_full()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .p_3()
            .gap_1()
            .border_r_1()
            .border_color(rgb(ui::LINE))
            .bg(rgb(0xfafbfc))
            .child(
                div()
                    .px_2()
                    .pt_3()
                    .pb_4()
                    .child(ui::caption("基础组件"))
                    .child(ui::hint("10 类 / 30 个交互样例")),
            )
            // Iterate lazily instead of constructing a large [Button; 10].
            .children(
                Family::ALL
                    .into_iter()
                    .map(|item| self.family_button(item, cx)),
            )
            .child(div().flex_1())
            .child(
                div()
                    .px_2()
                    .py_3()
                    .border_t_1()
                    .border_color(rgb(ui::LINE))
                    .child(ui::hint("原生 GPUI · 模拟样例"))
                    .child(ui::hint("选择自动保存到本机")),
            )
            .into_any_element()
    }

    fn header(&self, cx: &mut Context<Self>) -> AnyElement {
        let selected_count = cx
            .global::<GalleryPreferences>()
            .choices
            .iter()
            .flatten()
            .count();
        div()
            .p_5()
            .pb_4()
            .flex_shrink_0()
            .bg(rgb(ui::SURFACE))
            .border_b_1()
            .border_color(rgb(ui::LINE))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(22.))
                            .font_weight(FontWeight::BOLD)
                            .child("基础组件实验室"),
                    )
                    .child(ui::hint(format!("已记录 {selected_count} / 10 类倾向")))
                    .child(
                        Button::new("gallery-copy-preferences")
                            .ghost()
                            .label(if self.copied {
                                "已复制选择结果"
                            } else {
                                "复制选择结果"
                            })
                            .on_click(cx.listener(|this, _, _, cx| {
                                let text = gallery_preferences::summary(cx);
                                cx.write_to_clipboard(ClipboardItem::new_string(text));
                                this.copied = true;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("gallery-clear-preferences")
                            .ghost()
                            .label("清除倾向")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.copied = false;
                                gallery_preferences::clear(cx);
                            })),
                    ),
            )
            .child(div().mt_2().child(ui::hint(
                "先比较组件的形态和操作方式，再决定整体风格。三案使用相同配色，样例可直接操作。",
            )))
            .child(
                div()
                    .mt_2()
                    .child(ui::hint(cx.global::<GalleryPreferences>().status.clone())),
            )
            .into_any_element()
    }

    fn content_toolbar(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .flex()
            .items_center()
            .gap_3()
            .mb_2()
            .child(
                div()
                    .flex_1()
                    .text_size(px(19.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(format!(
                        "{:02}  {}",
                        self.family.index() + 1,
                        self.family.label()
                    )),
            )
            .child(
                Button::new("gallery-compare")
                    .ghost()
                    .selected(self.solo.is_none())
                    .label("三案并看")
                    .on_click(cx.listener(|this, _, window, cx| {
                        window.focus(&this.focus, cx);
                        this.solo = None;
                        cx.notify();
                    })),
            )
            .children(Variant::ALL.into_iter().map(|variant| {
                Button::new(format!("gallery-solo-{}", variant.key()))
                    .ghost()
                    .selected(self.solo == Some(variant))
                    .label(format!("仅 {}", variant.letter()))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        window.focus(&this.focus, cx);
                        this.solo = Some(variant);
                        cx.notify();
                    }))
                    .into_any_element()
            }))
            .into_any_element()
    }

    fn content(&self, cx: &mut Context<Self>) -> AnyElement {
        let toolbar = self.content_toolbar(cx);
        let columns = div()
            .flex()
            .items_start()
            .gap_4()
            .children(
                Variant::ALL
                    .into_iter()
                    .filter(|variant| self.solo.is_none_or(|selected| selected == *variant))
                    .map(|variant| self.column(variant, cx)),
            )
            .into_any_element();
        div()
            .id("gallery-content-scroll")
            .test_support()
            .track_scroll(&self.scroll)
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .p_5()
            .child(toolbar)
            .child(div().mb_5().child(ui::hint(self.family.purpose())))
            .child(columns)
            .child(
                div()
                    .mt_6()
                    .py_3()
                    .border_t_1()
                    .border_color(rgb(ui::LINE))
                    .child(ui::hint(
                        "比较重点：第一眼是否清楚 · 鼠标与键盘操作是否顺手 · 密集使用是否疲劳 · 错误和取消是否可预期",
                    )),
            )
            .into_any_element()
    }
}

impl Render for ComponentGallery {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let sidebar = self.sidebar(cx);
        let header = self.header(cx);
        let content = self.content(cx);
        div()
            .id("component-gallery")
            .test_support()
            .track_focus(&self.focus)
            .size_full()
            .min_h_0()
            .flex()
            .bg(rgb(ui::BG))
            .text_color(rgb(ui::INK))
            .text_size(px(14.))
            .child(sidebar)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .min_h_0()
                    .h_full()
                    .flex()
                    .flex_col()
                    .child(header)
                    .child(content),
            )
    }
}

/// An isolated entry point for design review. It never opens personal layouts.
pub fn run() {
    gpui_kit::application()
        .with_assets(crate::desktop_ui::AppAssets)
        .run(|cx| {
            crate::workspace::init(cx, false);
            // Persist only the design choices; personal workspaces stay closed.
            gallery_preferences::init(true, cx);
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();
            if let Err(error) = open(cx) {
                eprintln!("Cannot open component gallery: {error:#}");
                cx.quit();
                return;
            }
            cx.activate(true);
        });
}

pub(crate) fn open(cx: &mut App) -> anyhow::Result<(AnyWindowHandle, Entity<ComponentGallery>)> {
    let (handle, view) = gpui_kit::open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(1440.), px(900.)),
                cx,
            ))),
            titlebar: Some(TitlebarOptions {
                title: Some("Tydaro · 基础组件实验室".into()),
                ..Default::default()
            }),
            window_min_size: Some(size(px(1080.), px(700.))),
            ..Default::default()
        },
        cx,
        |window, cx| cx.new(|cx| ComponentGallery::new(window, cx)),
    )?;
    Ok((handle, view))
}
