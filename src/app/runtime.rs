//! Own long-lived tasks explicitly; dropping the runtime cancels its timers.
use gpui_kit::{App, Global, Task};
use std::time::Duration;

#[derive(Default)]
struct AppRuntime {
    market: Option<Task<()>>,
    layout: Option<Task<()>>,
}
impl Global for AppRuntime {}

pub(crate) fn init(cx: &mut App) {
    cx.set_global(AppRuntime::default());
}

pub(crate) fn start(cx: &mut App) {
    stop(cx);
    let market = cx.spawn(async move |cx| {
        loop {
            cx.background_executor().timer(Duration::from_secs(1)).await;
            cx.update(crate::workspace::advance_market);
        }
    });
    let layout = cx.spawn(async move |cx| {
        loop {
            cx.background_executor().timer(Duration::from_secs(1)).await;
            cx.update(crate::workspace::maintain_layout);
        }
    });
    let runtime = cx.global_mut::<AppRuntime>();
    runtime.market = Some(market);
    runtime.layout = Some(layout);
}

pub(crate) fn stop(cx: &mut App) {
    let runtime = cx.global_mut::<AppRuntime>();
    runtime.market = None;
    runtime.layout = None;
}
