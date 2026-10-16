//! Pointer/keyboard inspection, viewport gestures and chart composition.
use super::*;

impl MarketChart {
    pub(super) fn clear_inspection(&mut self) {
        self.inspecting = false;
        self.inspection = None;
        self.details = None;
        self.drag = None;
    }

    pub(super) fn selected_index(&self) -> Option<usize> {
        let selected = self.inspection.as_ref()?;
        selected.index_in(&self.data)
    }

    pub(super) fn data_range(&self) -> Range<usize> {
        if self.period == Period::Intraday {
            0..self.data.len()
        } else {
            self.viewport.range(self.data.len())
        }
    }

    pub(super) fn pointer_index(&self, pointer: Point<Pixels>) -> Option<usize> {
        let geometry = Geometry::new(self.bounds.get());
        if self.period == Period::Intraday {
            SessionTimeline::new(self.axis).index(geometry, pointer, &self.data)
        } else {
            let range = self.data_range();
            geometry
                .index(pointer, range.len())
                .map(|index| range.start + index)
        }
    }

    pub(super) fn locate(&mut self, pointer: Point<Pixels>) -> Option<usize> {
        let geometry = Geometry::new(self.bounds.get());
        let index = self.pointer_index(pointer)?;
        let on_volume = pointer.y.as_f32() >= geometry.volume_top;
        let (top, height) = if on_volume {
            (geometry.volume_top, geometry.bottom - geometry.volume_top)
        } else {
            (geometry.top, geometry.price_height)
        };
        let changed = self.selected_index() != Some(index);
        self.inspecting = true;
        self.inspection = Some(Inspection {
            time: self.data[index].time.clone(),
            y_fraction: ((pointer.y.as_f32() - top) / height.max(1.)).clamp(0., 1.),
            on_volume,
        });
        if changed {
            self.refresh_details();
        }
        Some(index)
    }

    pub(super) fn refresh_details(&mut self) {
        self.details = self.selected_index().and_then(|index| {
            if self.period == Period::Intraday {
                return None;
            }
            if self.period == Period::Day {
                let session = self
                    .source
                    .historical(self.symbol, index, self.tick)?
                    .session;
                Some(CandleDetails {
                    date: session.date,
                    weekday: session.weekday.into(),
                    bar: session.daily,
                    previous_close: Some(session.previous_close),
                    turnover: Some(session.turnover),
                    turnover_rate: Some(session.turnover_rate),
                })
            } else {
                Some(CandleDetails {
                    date: self.data[index].time.clone(),
                    weekday: String::new(),
                    bar: self.data[index].clone(),
                    previous_close: index
                        .checked_sub(1)
                        .map(|previous| self.data[previous].close),
                    turnover: None,
                    turnover_rate: None,
                })
            }
        });
    }

    pub(super) fn step_inspection(&mut self, right: bool) {
        let Some(index) = self.selected_index() else {
            return;
        };
        let next = if right {
            (index + 1).min(self.data.len() - 1)
        } else {
            index.saturating_sub(1)
        };
        if let Some(inspection) = self.inspection.as_mut() {
            inspection.time = self.data[next].time.clone();
        }
        if self.period != Period::Intraday {
            self.viewport.reveal(next, self.data.len());
        }
        self.refresh_details();
    }
}

impl Render for MarketChart {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let range = self.data_range();
        let visible = self.data[range.clone()].to_vec();
        let indicators = self.indicators.slice(range.clone());
        let geometry = Geometry::new(self.bounds.get());
        let selection = self
            .selected_index()
            .filter(|index| range.contains(index))
            .map(|index| index - range.start);
        let selected_index = selection.unwrap_or_else(|| visible.len().saturating_sub(1));
        let selected = visible.get(selected_index);
        let intraday = self.period == Period::Intraday;
        let reference_price = self.reference_price;
        let header = selected
            .map(|bar| {
                if intraday {
                    format!(
                        "{}  {}  {} {:.3}",
                        self.symbol.name(),
                        bar.time,
                        if selection.is_some() {
                            "价格"
                        } else {
                            "最新"
                        },
                        bar.close,
                    )
                } else {
                    format!(
                        "{}  {}  开 {:.2}  高 {:.2}  低 {:.2}  收 {:.2}",
                        self.symbol.name(),
                        bar.time,
                        bar.open,
                        bar.high,
                        bar.low,
                        bar.close
                    )
                }
            })
            .unwrap_or_else(|| format!("{}  等待成交", self.symbol.name()));
        let phase = self.phase.label().to_owned();
        let time_label = market::minute_text(self.wall_minute);
        let latest_time = self
            .data
            .last()
            .map(|bar| bar.time.clone())
            .unwrap_or_else(|| "尚未成交".into());
        let timeline = intraday.then(|| SessionTimeline::new(self.axis));
        let timeline_label = timeline
            .as_ref()
            .map(SessionTimeline::legend)
            .unwrap_or_default();
        let indicative = self.indicative.clone();
        let selected_time = selection
            .and_then(|index| visible.get(index))
            .map(|bar| bar.time.clone());
        let selected_average = indicators.session.get(selected_index).copied();
        let bounds_cell = self.bounds.clone();
        let crosshair = selection.and_then(|index| {
            self.inspection.as_ref().map(|inspection| Crosshair {
                index,
                y_fraction: inspection.y_fraction,
                on_volume: inspection.on_volume,
            })
        });
        let selected_date = (self.period == Period::Day)
            .then(|| self.details.as_ref().map(|details| details.date.clone()))
            .flatten();
        let details_card = selection
            .zip(self.details.as_ref())
            .map(|(index, details)| inspection_card(details, geometry, index, visible.len()));
        let intraday_card = selection.and_then(|index| {
            let timeline = timeline.as_ref()?;
            let bar = visible.get(index)?;
            let details =
                IntradayDetails::new(bar, reference_price, indicators.session.get(index).copied());
            let x = timeline.x(market::bar_wall_minute(bar), geometry);
            Some(intraday_inspection_card(
                details,
                geometry,
                x,
                self.symbol.currency(),
                cx,
            ))
        });
        let latest = self.data.last().map(|b| b.close);

        div()
            .flex()
            .flex_col()
            .size_full()
            .min_h_0()
            .bg(rgb(0xFFFFFF))
            .child(
                div()
                    .h(px(23.))
                    .flex_shrink_0()
                    .px_3()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_size(px(10.))
                    .text_color(rgb(MUTED))
                    .overflow_hidden()
                    .child(
                        div()
                            .id("chart-price-header")
                            .test_support()
                            .aria_label(header.clone())
                            .flex_shrink_0()
                            .text_color(rgb(BLUE))
                            .child(header),
                    )
                    .when(intraday, |row| {
                        row.child(
                            div().flex_shrink_0().text_color(rgb(VWAP)).child(
                                selected_average
                                    .map(|value| format!("均价 {value:.3}"))
                                    .unwrap_or_else(|| "均价 —".into()),
                            ),
                        )
                    }),
            )
            .when(intraday, |chart| {
                chart.child(
                    div()
                        .h(px(22.))
                        .flex_shrink_0()
                        .px_3()
                        .flex()
                        .items_center()
                        .gap_2()
                        .overflow_hidden()
                        .text_size(px(10.))
                        .text_color(rgb(MUTED))
                        .child(
                            div()
                                .id("intraday-session-phase")
                                .test_support()
                                .aria_label(phase.clone())
                                .flex_shrink_0()
                                .text_color(rgb(BLUE))
                                .child(phase),
                        )
                        .child(
                            div()
                                .id("intraday-session-time")
                                .test_support()
                                .aria_label(time_label.clone())
                                .flex_shrink_0()
                                .child(time_label),
                        )
                        .child(div().flex_shrink_0().child("最近成交"))
                        .child(
                            div()
                                .id("intraday-last-time")
                                .test_support()
                                .aria_label(latest_time.clone())
                                .flex_shrink_0()
                                .child(latest_time),
                        )
                        .child(
                            div()
                                .id("intraday-timeline-label")
                                .test_support()
                                .aria_label(timeline_label.clone())
                                .truncate()
                                .child(timeline_label),
                        ),
                )
            })
            .child(
                div()
                    .id("native-market-chart")
                    .test_support()
                    .track_focus(&self.focus)
                    .on_key_down(cx.listener(|this, event: &gpui_kit::KeyDownEvent, _, cx| {
                        if !this.inspecting {
                            return;
                        }
                        match event.keystroke.key.as_str() {
                            "escape" => this.clear_inspection(),
                            "left" => this.step_inspection(false),
                            "right" => this.step_inspection(true),
                            _ => return,
                        }
                        cx.stop_propagation();
                        cx.notify();
                    }))
                    .relative()
                    .flex_1()
                    .min_h(px(80.))
                    .overflow_hidden()
                    .on_mouse_move(
                        cx.listener(|this, event: &gpui_kit::MouseMoveEvent, _, cx| {
                            if let Some(mut drag) = this.drag {
                                if event.dragging() {
                                    let dx = event.position.x.as_f32() - drag.origin.x.as_f32();
                                    let dy = event.position.y.as_f32() - drag.origin.y.as_f32();
                                    drag.moved |= dx.hypot(dy) > 4.;
                                    if drag.moved && this.period != Period::Intraday {
                                        let geometry = Geometry::new(this.bounds.get());
                                        let count = drag.viewport.range(this.data.len()).len();
                                        this.viewport = drag.viewport;
                                        this.viewport.pan(
                                            (dx / geometry.width.max(1.) * count as f32).round()
                                                as isize,
                                            this.data.len(),
                                        );
                                    }
                                    this.drag = Some(drag);
                                    cx.notify();
                                    return;
                                }
                                this.drag = None;
                            }
                            if this.inspecting {
                                if this.locate(event.position).is_none()
                                    && this.period == Period::Intraday
                                {
                                    // Empty future/auction/pause positions are not aliases for the latest trade.
                                    this.inspection = None;
                                    this.details = None;
                                }
                                cx.notify();
                            }
                        }),
                    )
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, event: &gpui_kit::MouseDownEvent, window, cx| {
                            if this.pointer_index(event.position).is_some() {
                                this.focus.focus(window, cx);
                                this.drag = Some(DragState {
                                    origin: event.position,
                                    viewport: this.viewport,
                                    moved: false,
                                    click_count: event.click_count,
                                    inspection_was_active: this.inspecting,
                                });
                            } else if this.period == Period::Intraday {
                                this.clear_inspection();
                                cx.notify();
                            }
                        }),
                    )
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|this, event: &gpui_kit::MouseUpEvent, _, cx| {
                            let Some(drag) = this.drag.take() else {
                                return;
                            };
                            let was_dragged = this.last_gesture_dragged;
                            this.last_gesture_dragged = drag.moved;
                            if !drag.moved {
                                if this.period == Period::Intraday && drag.inspection_was_active {
                                    // A second click dismisses the whole inspection, even on another minute.
                                    this.clear_inspection();
                                } else if let Some(index) = this.locate(event.position)
                                    && drag.click_count == 2
                                    && !was_dragged
                                    && this.period == Period::Day
                                    && !this.historical
                                {
                                    cx.emit(ChartEvent::InspectDay {
                                        symbol: this.symbol,
                                        day_index: index,
                                        tick: this.tick,
                                    });
                                }
                            }
                            cx.notify();
                        }),
                    )
                    .on_hover(cx.listener(|this, hovered, _, _| {
                        if !hovered {
                            this.drag = None;
                        }
                    }))
                    .on_scroll_wheel(cx.listener(
                        |this, event: &gpui_kit::ScrollWheelEvent, _, cx| {
                            if this.period == Period::Intraday {
                                cx.stop_propagation();
                                return;
                            }
                            let (x, y) = match event.delta {
                                ScrollDelta::Pixels(p) => (p.x.as_f32(), p.y.as_f32()),
                                ScrollDelta::Lines(p) => (p.x * 24., p.y * 24.),
                            };
                            if event.modifiers.shift || x.abs() > y.abs() {
                                let amount = if x.abs() > y.abs() { x } else { y };
                                this.viewport
                                    .pan((amount / 8.).round() as isize, this.data.len());
                            } else if y.abs() > 0.1 {
                                let geometry = Geometry::new(this.bounds.get());
                                let anchor = (event.position.x.as_f32() - geometry.left)
                                    / geometry.width.max(1.);
                                this.viewport.zoom(y > 0., anchor, this.data.len());
                            }
                            cx.stop_propagation();
                            cx.notify();
                        },
                    ))
                    .child(
                        canvas(
                            move |bounds, _, _| bounds_cell.set(bounds),
                            move |bounds, _, window, cx| {
                                paint_chart(
                                    bounds,
                                    &visible,
                                    &indicators,
                                    intraday,
                                    timeline.as_ref(),
                                    &indicative,
                                    reference_price,
                                    latest,
                                    crosshair,
                                    selected_date.as_deref(),
                                    window,
                                    cx,
                                );
                            },
                        )
                        .size_full(),
                    )
                    .children(details_card)
                    .children(intraday_card)
                    .children(selected_time.map(|time| {
                        div()
                            .id("chart-selected-time")
                            .test_support()
                            .aria_label(time.clone())
                            .absolute()
                            .top_0()
                            .right(px(58.))
                            .text_size(px(10.))
                            .text_color(rgb(MUTED))
                            .child(time)
                    })),
            )
    }
}
