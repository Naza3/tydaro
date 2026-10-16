//! Native five-page shell. Pages retain entities; market DockArea retains ownership.
use crate::{
    board_bar::BoardBar,
    chart::MarketChart,
    desktop_state::{self, DEMO_DATE, DesktopEvent, DesktopState, PageId, SourceContext},
    desktop_ui::*,
    journal_page::JournalPage,
    market::{self, Period, Symbol},
    research_page::{LimitPage, ThemePage},
    watch_page::WatchPage,
    workspace,
};
use gpui_kit::{
    assets::IconName,
    component::{
        Disableable, Icon, Selectable, Sizable, WindowExt,
        button::{Button, ButtonVariants},
        dock::DockArea,
        input::{Input, InputEvent, InputState},
    },
    prelude::*,
    *,
};
use std::cell::Cell;

#[derive(Clone)]
enum Overlay {
    Search,
    Inspect(SourceContext),
    Watch(SourceContext),
    Draft(SourceContext),
    Help,
}

pub struct Desktop {
    dock: Entity<DockArea>,
    board_bar: Option<Entity<BoardBar>>,
    watch: Entity<WatchPage>,
    limit: Entity<LimitPage>,
    theme: Entity<ThemePage>,
    journal: Entity<JournalPage>,
    search: Entity<InputState>,
    draft_date: Entity<InputState>,
    chart: Entity<MarketChart>,
    chart_period: Period,
    overlay: Option<Overlay>,
    target_group: String,
    notice: String,
    focus: FocusHandle,
    _subscriptions: Vec<Subscription>,
}
impl Desktop {
    pub fn new(
        dock: Entity<DockArea>,
        board_bar: Option<Entity<BoardBar>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let watch = cx.new(|cx| WatchPage::new(window, cx));
        let limit = cx.new(|cx| LimitPage::new(window, cx));
        let theme = cx.new(|cx| ThemePage::new(window, cx));
        let journal = cx.new(|cx| JournalPage::new(window, cx));
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("搜索股票、代码或页面"));
        let draft_date = cx.new(|cx| InputState::new(window, cx).placeholder("YYYY-MM-DD"));
        let mut subscriptions = vec![
            cx.subscribe_in(
                &watch,
                window,
                |this, _, event: &DesktopEvent, window, cx| {
                    this.dispatch(event.clone(), window, cx)
                },
            ),
            cx.subscribe_in(
                &limit,
                window,
                |this, _, event: &DesktopEvent, window, cx| {
                    this.dispatch(event.clone(), window, cx)
                },
            ),
            cx.subscribe_in(
                &theme,
                window,
                |this, _, event: &DesktopEvent, window, cx| {
                    this.dispatch(event.clone(), window, cx)
                },
            ),
            cx.subscribe_in(
                &journal,
                window,
                |this, _, event: &DesktopEvent, window, cx| {
                    this.dispatch(event.clone(), window, cx)
                },
            ),
        ];
        subscriptions.push(cx.subscribe_in(
            &search,
            window,
            |this, _, event, window, cx| match event {
                InputEvent::PressEnter { .. } => this.submit_search(window, cx),
                InputEvent::Change => cx.refresh_windows(),
                _ => {}
            },
        ));
        subscriptions
            .push(cx.subscribe(&draft_date, |_, _, _: &InputEvent, cx| cx.refresh_windows()));
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        Self {
            dock,
            board_bar,
            watch,
            limit,
            theme,
            journal,
            search,
            draft_date,
            chart: cx.new(|cx| MarketChart::new(Symbol::Yinli, Period::Intraday, cx)),
            chart_period: Period::Intraday,
            overlay: None,
            target_group: String::new(),
            notice: String::new(),
            focus,
            _subscriptions: subscriptions,
        }
    }
    fn page(&self, cx: &App) -> PageId {
        cx.global::<DesktopState>().active_page
    }
    fn editing(&self, cx: &App) -> bool {
        match self.page(cx) {
            PageId::Watch => self.watch.read(cx).is_editing(),
            PageId::Limit => self.limit.read(cx).is_editing(),
            PageId::Theme => self.theme.read(cx).is_editing(),
            PageId::Journal => self.journal.read(cx).is_editing(),
            PageId::Market => false,
        }
    }
    fn navigate(&mut self, page: PageId, cx: &mut Context<Self>) {
        if page != self.page(cx) && self.editing(cx) {
            self.notice = "请先保存或取消当前编辑，再切换页面".into();
            cx.notify();
            return;
        }
        desktop_state::update(cx, |state| state.active_page = page);
        self.notice.clear();
        cx.notify();
    }
    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.overlay = None;
        window.close_dialog(cx);
        cx.notify();
    }
    fn open(&mut self, overlay: Overlay, window: &mut Window, cx: &mut Context<Self>) {
        if self.overlay.is_some() {
            window.close_dialog(cx);
        }
        self.overlay = Some(overlay);
        let owner = cx.weak_entity();
        let closed = owner.clone();
        let first = Cell::new(true);
        window.open_dialog(cx, move |dialog, window, cx| {
            let rendered = owner.update(cx, |this, cx| {
                if first.replace(false) {
                    match this.overlay {
                        Some(Overlay::Search) => {
                            this.search.update(cx, |input, cx| input.focus(window, cx))
                        }
                        Some(Overlay::Draft(_)) => this
                            .draft_date
                            .update(cx, |input, cx| input.focus(window, cx)),
                        _ => {}
                    }
                }
                let (title, width) = match &this.overlay {
                    Some(Overlay::Search) => ("全局搜索", 620.),
                    Some(Overlay::Inspect(_)) => ("个股速览", 790.),
                    Some(Overlay::Watch(_)) => ("加入盯盘分组", 600.),
                    Some(Overlay::Draft(_)) => ("引用为复盘草稿", 600.),
                    _ => ("Tydaro · 原生工作台", 600.),
                };
                (title, width, this.overlay_content(window, cx))
            });
            if let Ok((title, width, content)) = rendered {
                let closed = closed.clone();
                dialog
                    .title(title)
                    .width(px(width))
                    .child(content)
                    .on_close(move |_, _, cx| {
                        let _ = closed.update(cx, |this, cx| {
                            this.overlay = None;
                            cx.notify();
                        });
                    })
            } else {
                dialog
            }
        });
        cx.notify();
    }
    fn begin_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.editing(cx)
            || (window.has_active_dialog(cx) && !matches!(self.overlay, Some(Overlay::Search)))
        {
            self.notice = "请先完成或关闭当前编辑，再使用全局搜索".into();
            cx.notify();
            return;
        }
        self.search
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.open(Overlay::Search, window, cx);
    }
    fn search_matches(&self, cx: &App) -> Vec<Symbol> {
        let q = self.search.read(cx).value().trim().to_lowercase();
        Symbol::ALL
            .into_iter()
            .filter(|s| {
                q.is_empty()
                    || format!("{} {} {}", s.name(), s.code(), s.sector())
                        .to_lowercase()
                        .contains(&q)
            })
            .collect()
    }
    fn submit_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !matches!(self.overlay, Some(Overlay::Search)) {
            return;
        }
        let query = self.search.read(cx).value().to_string();
        if let Some(symbol) = self.search_matches(cx).first().copied() {
            self.dispatch(DesktopEvent::OpenMarket(symbol), window, cx);
        } else if let Some(page) = PageId::ALL
            .into_iter()
            .find(|p| p.label().contains(query.trim()))
        {
            self.close(window, cx);
            self.navigate(page, cx);
        }
    }
    fn dispatch(&mut self, event: DesktopEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.editing(cx) && !matches!(&event, DesktopEvent::Notice(_)) {
            self.notice = "请先保存或取消当前编辑，再打开其他资料".into();
            cx.notify();
            return;
        }
        match event {
            DesktopEvent::Inspect(context) => {
                if let Some(symbol) = context.symbols.first().copied() {
                    let tick = workspace::market_tick(cx);
                    self.chart_period = Period::Intraday;
                    self.chart.update(cx, |chart, cx| {
                        chart.set_market(symbol, Period::Intraday, tick, cx)
                    });
                    self.open(Overlay::Inspect(context), window, cx);
                }
            }
            DesktopEvent::OpenMarket(symbol) => {
                if self.overlay.is_some() {
                    self.close(window, cx);
                }
                self.navigate(PageId::Market, cx);
                cx.defer(move |cx| workspace::open_market_symbol(symbol, cx));
            }
            DesktopEvent::AddToWatch(context) => {
                self.target_group = cx
                    .global::<DesktopState>()
                    .watch_groups
                    .first()
                    .map(|g| g.id.clone())
                    .unwrap_or_default();
                self.open(Overlay::Watch(context), window, cx);
            }
            DesktopEvent::CreateDraft(context) => {
                self.draft_date.update(cx, |input, cx| {
                    input.set_value(context.date.clone(), window, cx)
                });
                self.open(Overlay::Draft(context), window, cx);
            }
            DesktopEvent::OpenTheme(context) => {
                self.theme
                    .update(cx, |theme, cx| theme.receive(context, window, cx));
                self.navigate(PageId::Theme, cx);
            }
            DesktopEvent::Notice(message) => {
                self.notice = message;
                cx.notify();
            }
        }
    }
    fn source(context: &SourceContext) -> Div {
        panel()
            .p_3()
            .flex()
            .flex_col()
            .gap_2()
            .bg(rgb(0xf3f8f8))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(tag(format!("来自{}", context.origin.label()), TEAL))
                    .child(context.sector.clone())
                    .child(div().flex_1())
                    .child(muted(format!("资料 {}", context.date))),
            )
            .child(muted(context.reason.clone()))
    }
    fn overlay_content(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let Some(overlay) = self.overlay.clone() else {
            return div().into_any_element();
        };
        match overlay {
            Overlay::Search => {
                let query = self.search.read(cx).value().trim().to_string();
                let matches = self.search_matches(cx);
                let pages = div().flex().gap_2().children(
                    PageId::ALL.into_iter().filter(|p| query.is_empty() || p.label().contains(&query)).map(|page| {
                        Button::new(format!("desktop-search-page-{}", page_key(page))).small().label(page.label())
                            .on_click(cx.listener(move |this, _, window, cx| { this.close(window,cx); this.navigate(page,cx); }))
                    })
                );
                let results = div().id("desktop-search-results").max_h(px(420.)).overflow_y_scroll().children(
                    matches.iter().map(|s| {
                        let symbol = *s;
                        Button::new(format!("desktop-search-result-{}",symbol.code())).ghost().w_full().h(px(46.))
                            .label(format!("{}   {}                         {}",symbol.name(),symbol.code(),symbol.sector()))
                            .on_click(cx.listener(move|this,_,window,cx|this.dispatch(DesktopEvent::OpenMarket(symbol),window,cx)))
                    })
                );
                div().flex().flex_col().gap_3()
                    .child(div().id("desktop-search-input").test_support().child(Input::new(&self.search)))
                    .child(pages).child(results)
                    .when(matches.is_empty(),|d|d.child(muted("没有匹配股票，可搜索页面名称")))
                    .child(muted("Enter 打开首个结果 · Esc 关闭 · 本地模拟目录")).into_any_element()
            }
            Overlay::Inspect(context)=>{
                let symbol=context.symbols[0];let tick=workspace::market_tick(cx);
                self.chart.update(cx,|chart,cx|chart.set_market(symbol,self.chart_period,tick,cx));
                let quote=market::quote(symbol,tick);let up=quote.change_pct>=0.;
                div().flex().flex_col().gap_3().child(Self::source(&context)).child(div().flex().items_center().justify_between().child(div().flex().flex_col().child(muted(format!("{} {}",symbol.market(),symbol.code()))).child(div().text_size(px(24.)).font_weight(FontWeight::BOLD).child(symbol.name()))).child(div().text_color(rgb(if up{UP}else{DOWN})).flex().flex_col().child(div().text_size(px(28.)).child(format!("{:.2}",quote.price))).child(format!("{:+.2}  {:+.2}%",quote.change,quote.change_pct))))
                .child(div().flex().gap_2().children([Period::Intraday,Period::Day].map(|period|Button::new(format!("desktop-inspect-{}",period.label())).small().label(period.label()).selected(self.chart_period==period).on_click(cx.listener(move|this,_,_,cx|{this.chart_period=period;let tick=workspace::market_tick(cx);this.chart.update(cx,|chart,cx|chart.set_market(symbol,period,tick,cx));cx.refresh_windows();})))).child(div().flex_1()).child(muted(format!("{} · 行情演示",DEMO_DATE))))
                .child(div().h(px(360.)).child(self.chart.clone())).child(div().flex().justify_end().gap_2().child(Button::new("desktop-inspect-close").label("返回研究").on_click(cx.listener(|this,_,window,cx|this.close(window,cx)))).child(Button::new("desktop-inspect-draft").label("引用到复盘").on_click(cx.listener({let context=context.clone();move|this,_,window,cx|this.dispatch(DesktopEvent::CreateDraft(context.clone()),window,cx)}))).child(Button::new("desktop-inspect-watch").label("加入盯盘").on_click(cx.listener(move|this,_,window,cx|this.dispatch(DesktopEvent::AddToWatch(context.clone()),window,cx)))).child(Button::new("desktop-inspect-market").primary().label("打开行情").on_click(cx.listener(move|this,_,window,cx|this.dispatch(DesktopEvent::OpenMarket(symbol),window,cx))))).into_any_element()
            }
            Overlay::Watch(context)=>{
                let groups=cx.global::<DesktopState>().watch_groups.clone();let target=groups.iter().find(|g|g.id==self.target_group);let added=context.symbols.iter().filter(|symbol|!target.is_some_and(|g|g.symbols.contains(symbol))).count();let exists=context.symbols.len()-added;
                div().flex().flex_col().gap_3().child(Self::source(&context)).child(muted("目标分组 · 只追加缺少的股票，保留原有成员"))
                .child(div().id("watch-target-list").max_h(px(180.)).overflow_y_scroll().children(groups.iter().map(|g|{let id=g.id.clone();Button::new(format!("watch-target-{}",g.id)).w_full().label(format!("{} · {} 只",g.name,g.symbols.len())).selected(g.id==self.target_group).on_click(cx.listener(move|this,_,_,cx|{this.target_group=id.clone();cx.refresh_windows();}))})))
                .child(div().flex().flex_col().gap_2().children(context.symbols.iter().map(|symbol|div().flex().justify_between().child(format!("{} {}",symbol.name(),symbol.code())).child(muted(if target.is_some_and(|g|g.symbols.contains(symbol)){"已存在 · 跳过"}else{"＋ 新增"})))))
                .child(muted(format!("新增 {added} 只 · 跳过 {exists} 只重复股票"))).child(div().flex().justify_end().gap_2().child(Button::new("watch-add-cancel").label("取消").on_click(cx.listener(|this,_,window,cx|this.close(window,cx)))).child(Button::new("watch-add-confirm").primary().label("确认加入").disabled(target.is_none()||added==0).on_click(cx.listener(move|this,_,window,cx|{let id=this.target_group.clone();let result=desktop_state::update(cx,|state|state.add_to_watch(&id,&context.symbols));match result{Ok(count)=>{this.close(window,cx);this.navigate(PageId::Watch,cx);this.notice=format!("已加入 {count} 只股票");},Err(error)=>this.notice=error.to_string()}cx.notify();})))).into_any_element()
            }
            Overlay::Draft(context)=>{
                let valid=desktop_state::date_valid(self.draft_date.read(cx).value().trim());
                div().flex().flex_col().gap_3().child(Self::source(&context)).child(muted("草稿业务日期 · YYYY-MM-DD"))
                .child(div().id("draft-business-date").test_support().child(Input::new(&self.draft_date))).child(muted("带入股票、题材及来源，买卖条件由你填写；新建独立草稿，保留已有预案。"))
                .child(div().flex().justify_end().gap_2().child(Button::new("draft-create-cancel").label("取消").on_click(cx.listener(|this,_,window,cx|this.close(window,cx)))).child(Button::new("draft-create-confirm").primary().label("创建草稿").disabled(!valid).on_click(cx.listener(move|this,_,window,cx|{let date=this.draft_date.read(cx).value().trim().to_string();if !desktop_state::date_valid(&date){return;}this.close(window,cx);this.journal.update(cx,|journal,cx|journal.receive_on_date(context.clone(),date,window,cx));this.navigate(PageId::Journal,cx);}))))
                .into_any_element()
            }
            Overlay::Help=>div().flex().flex_col().gap_3().child(div().text_size(px(20.)).font_weight(FontWeight::BOLD).child("从发现线索到形成预案"))
                .child(muted("行情 · 保留看板与独立组件；盯盘 · 分组与异动；涨停 · 跨日资料；题材 · 周期证据；复盘 · 预案与阅读窗口。"))
                .child(muted("Ctrl K 搜索股票与页面。日线双击查看历史分时；单击图表开关十字线。新增行情组件默认独立窗口，可重新插入看板。"))
                .child(muted("行情、异动与研究证据为本地模拟。分组、资料编辑和复盘使用独立本地文件保存；布局继续使用原有看板文件。"))
                .child(Button::new("desktop-help-close").primary().label("开始使用").on_click(cx.listener(|this,_,window,cx|this.close(window,cx)))).into_any_element(),
        }
    }
}
fn page_key(page: PageId) -> &'static str {
    match page {
        PageId::Market => "market",
        PageId::Watch => "watch",
        PageId::Limit => "limit",
        PageId::Theme => "theme",
        PageId::Journal => "journal",
    }
}
fn page_icon(page: PageId) -> IconName {
    match page {
        PageId::Market => IconName::LayoutDashboard,
        PageId::Watch => IconName::Activity,
        PageId::Limit => IconName::TrendingUp,
        PageId::Theme => IconName::Layers,
        PageId::Journal => IconName::BookOpen,
    }
}
impl Render for Desktop {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let page = self.page(cx);
        let state = cx.global::<DesktopState>();
        let unread = state.unread_count();
        let monitoring = state.monitoring;
        let save_status = state.save_status.clone();
        let nav =
            div()
                .w(px(78.))
                .h_full()
                .flex_shrink_0()
                .border_r_1()
                .border_color(rgb(LINE))
                .bg(rgb(0xf9fafc))
                .p_2()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .h(px(60.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(px(29.))
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(TEAL))
                        .child("▥"),
                )
                .child(
                    div()
                        .text_size(px(10.))
                        .text_color(rgb(MUTED))
                        .text_center()
                        .child("工 作 台"),
                )
                .children(PageId::ALL.map(|target| {
                    Button::new(format!("desktop-nav-{}", page_key(target)))
                        .ghost()
                        .h(px(64.))
                        .w_full()
                        .selected(page == target)
                        .when(page == target, |this| this.text_color(rgb(TEAL)))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .items_center()
                                .gap_1()
                                .child(Icon::new(page_icon(target)).size(px(21.)))
                                .child(div().text_size(px(12.)).child(target.label())),
                        )
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.navigate(target, cx);
                            if !this.editing(cx) {
                                window.focus(&this.focus, cx);
                            }
                        }))
                }))
                .child(div().flex_1())
                .child(Button::new("desktop-help").ghost().label("帮助").on_click(
                    cx.listener(|this, _, window, cx| this.open(Overlay::Help, window, cx)),
                ))
                .child(div().p_2().text_center().text_color(rgb(MUTED)).child("T"));
        let header = div()
            .h(px(62.))
            .flex_shrink_0()
            .px_5()
            .flex()
            .items_center()
            .gap_4()
            .border_b_1()
            .border_color(rgb(LINE))
            .bg(rgb(0xffffff))
            .child(
                div()
                    .flex()
                    .items_baseline()
                    .text_size(px(25.))
                    .font_weight(FontWeight::BOLD)
                    .child("tydaro")
                    .child(div().text_color(rgb(TEAL)).child(".")),
            )
            .child(muted("│"))
            .child(div().font_weight(FontWeight::SEMIBOLD).child(page.label()))
            .child(div().flex_1())
            .child(
                Button::new("desktop-search-trigger")
                    .ghost()
                    .w(px(290.))
                    .icon(IconName::Search)
                    .label("搜索股票、代码或页面     Ctrl K")
                    .on_click(cx.listener(|this, _, window, cx| this.begin_search(window, cx))),
            )
            .child(tag("原生预览", TEAL))
            .child(
                Button::new("desktop-notifications")
                    .ghost()
                    .icon(IconName::Bell)
                    .label(unread.to_string())
                    .on_click(cx.listener(|this, _, _, cx| this.navigate(PageId::Watch, cx))),
            );
        let content = match page {
            PageId::Market => div()
                .size_full()
                .min_h_0()
                .flex()
                .flex_col()
                .child(workspace::market_toolbar(&self.board_bar, &self.dock, cx))
                .child(
                    div()
                        .h(px(46.))
                        .flex_shrink_0()
                        .px_4()
                        .flex()
                        .items_center()
                        .gap_6()
                        .border_b_1()
                        .border_color(rgb(LINE))
                        .children(
                            [
                                ("上证指数", "3,962.84", "+0.38%"),
                                ("深证成指", "13,184.72", "+0.72%"),
                                ("创业板指", "3,067.18", "+1.26%"),
                                ("科创 50", "1,426.35", "−0.21%"),
                            ]
                            .map(|(name, value, change)| {
                                div()
                                    .flex_1()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .gap_2()
                                    .child(
                                        div().flex().flex_col().child(muted(name)).child(
                                            div()
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(rgb(if change.starts_with('−') {
                                                    DOWN
                                                } else {
                                                    UP
                                                }))
                                                .child(value),
                                        ),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(12.))
                                            .text_color(rgb(if change.starts_with('−') {
                                                DOWN
                                            } else {
                                                UP
                                            }))
                                            .child(change),
                                    )
                            }),
                        ),
                )
                .child(
                    div()
                        .flex_1()
                        .min_h_0()
                        .relative()
                        .child(
                            canvas(
                                |bounds, window, cx| {
                                    workspace::register_main_dock(bounds, window, cx)
                                },
                                |_, _, _, _| {},
                            )
                            .absolute()
                            .inset_0()
                            .size_full(),
                        )
                        .child(self.dock.clone()),
                )
                .into_any_element(),
            PageId::Watch => self.watch.clone().into_any_element(),
            PageId::Limit => self.limit.clone().into_any_element(),
            PageId::Theme => self.theme.clone().into_any_element(),
            PageId::Journal => self.journal.clone().into_any_element(),
        };
        div()
            .id("native-desktop")
            .test_support()
            .track_focus(&self.focus)
            .size_full()
            .relative()
            .flex()
            .bg(rgb(BG))
            .text_color(rgb(INK))
            .text_size(px(14.))
            .capture_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if event.keystroke.modifiers.control
                    && event.keystroke.key.eq_ignore_ascii_case("k")
                {
                    this.begin_search(window, cx);
                    cx.stop_propagation();
                }
            }))
            .child(nav)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .flex()
                    .flex_col()
                    .child(header)
                    .child(div().flex_1().min_h_0().child(content))
                    .child(
                        div()
                            .h(px(28.))
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .px_4()
                            .gap_3()
                            .bg(rgb(0xffffff))
                            .border_t_1()
                            .border_color(rgb(LINE))
                            .text_size(px(11.))
                            .text_color(rgb(MUTED))
                            .child(div().text_color(rgb(TEAL)).child(if monitoring {
                                "● 演示监控已启用"
                            } else {
                                "○ 演示监控已暂停"
                            }))
                            .child("A 股 · 模拟数据")
                            .child(DEMO_DATE)
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .child(self.notice.clone()),
                            )
                            .child(
                                div()
                                    .max_w(px(350.))
                                    .truncate()
                                    .child(workspace::status_message(cx)),
                            )
                            .child(div().flex_shrink_0().child(save_status)),
                    ),
            )
    }
}
