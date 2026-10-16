//! Explicit local research samples and presentation helpers shared by the two research pages.
//! They never infer live theme strength from edited imported rows.
use crate::app::ui::{LINE, TEAL, panel as base_panel};
use crate::market::Symbol;
use gpui_kit::{component::button::Button, prelude::*, *};
use tydaro_core::research::ResearchRow;

pub(super) const DATES: [&str; 6] = [
    "2026-10-14",
    "2026-10-13",
    "2026-10-12",
    "2026-10-09",
    "2026-10-08",
    "2026-10-07",
];
pub(super) const EVIDENCE_DATES: [&str; 10] = [
    "2026-10-14",
    "2026-10-13",
    "2026-10-12",
    "2026-10-09",
    "2026-10-08",
    "2026-10-07",
    "2026-09-30",
    "2026-09-29",
    "2026-09-28",
    "2026-09-25",
];
pub(super) const SECTORS: [&str; 4] = ["AI 应用", "汽车产业链", "消费复苏", "科技硬件"];
const ORDERS: [[usize; 4]; 6] = [
    [0, 1, 2, 3],
    [1, 0, 3, 2],
    [2, 3, 0, 1],
    [0, 3, 1, 2],
    [3, 2, 0, 1],
    [1, 2, 3, 0],
];

fn sector_symbols(sector: usize) -> Vec<Symbol> {
    match sector {
        0 => vec![Symbol::Yinli, Symbol::Eastmoney, Symbol::Luxshare],
        1 => vec![Symbol::Seres, Symbol::Byd, Symbol::Catl],
        2 => vec![
            Symbol::Yangguang,
            Symbol::Moutai,
            Symbol::Wuliangye,
            Symbol::Midea,
        ],
        _ => vec![Symbol::Smic, Symbol::Luxshare, Symbol::Midea],
    }
}
fn reason(symbol: Symbol) -> &'static str {
    match symbol {
        Symbol::Yinli => "AI 营销 + 数字传媒 + 内容生产",
        Symbol::Eastmoney => "互联网金融 + AI 应用",
        Symbol::Luxshare => "消费电子 + AI 终端 + 产业链",
        Symbol::Seres => "新能源汽车 + 智能驾驶",
        Symbol::Byd => "新能源汽车 + 海外布局",
        Symbol::Catl => "动力电池 + 储能",
        Symbol::Yangguang => "乳业 + 消费复苏 + 区域品牌",
        Symbol::Moutai | Symbol::Wuliangye => "白酒 + 消费复苏",
        Symbol::Midea => "家用电器 + 智能制造",
        Symbol::Smic => "半导体 + 国产替代",
        _ => "示例研究资料",
    }
}
pub(super) fn sector_reason(sector: &str) -> &'static str {
    match sector {
        "AI 应用" => "内容生产与营销应用受到关注，观察核心标的与板块跟随的持续性。",
        "汽车产业链" => "新车型与产业链需求预期共振，关注整车及电池方向的承接。",
        "消费复苏" => "消费预期延续，乳业、白酒及家电呈现轮动，关注量能变化。",
        _ => "国产替代与终端创新预期带动关注，观察半导体与消费电子联动。",
    }
}
pub(super) fn sector_color(sector: &str) -> u32 {
    match sector {
        "AI 应用" => TEAL,
        "汽车产业链" => 0x6686c0,
        "消费复苏" => 0xb58a50,
        _ => 0x987cac,
    }
}
pub(super) fn sample_rows(date: &str, day: usize) -> Vec<ResearchRow> {
    ORDERS[day % ORDERS.len()]
        .iter()
        .enumerate()
        .flat_map(|(position, sector)| {
            let count = if (day + position) % 3 == 1 { 2 } else { 3 };
            sector_symbols(*sector)
                .into_iter()
                .take(count)
                .enumerate()
                .map(move |(index, symbol)| ResearchRow {
                    id: format!("{date}-{}-{}", SECTORS[*sector], symbol.code()),
                    date: date.into(),
                    sector: SECTORS[*sector].into(),
                    symbol,
                    name: symbol.name().into(),
                    board: if index == 0 {
                        ["4天4板", "3天3板", "2天2板", "首板"][day % 4]
                    } else if index == 1 {
                        "2天2板"
                    } else {
                        "首板"
                    }
                    .into(),
                    reason: reason(symbol).into(),
                    source: format!("{date}_涨停复盘_示例.csv"),
                })
        })
        .collect()
}
pub(super) fn seed_rows() -> Vec<ResearchRow> {
    DATES
        .iter()
        .enumerate()
        .flat_map(|(day, date)| sample_rows(date, day))
        .collect()
}
pub(super) fn button(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Button {
    tydaro_ui::action::action(id, label, tydaro_ui::action::ActionSize::Small).flex_shrink_0()
}
pub(super) fn panel() -> Div {
    // Pages scroll as a whole. Intrinsic cards must retain their content height
    // instead of shrinking and clipping inputs or the last imported-day row.
    base_panel().flex_shrink_0()
}
pub(super) fn text_cell(text: impl Into<SharedString>, width: f32) -> Div {
    div()
        .w(px(width))
        .flex_shrink_0()
        .overflow_hidden()
        .child(text.into())
}
pub(super) fn line() -> Div {
    div().h(px(1.)).w_full().bg(rgb(LINE))
}
pub(super) fn section_title(title: impl Into<SharedString>) -> Div {
    div()
        .h(px(46.))
        .px_4()
        .flex()
        .items_center()
        .gap_2()
        .border_b_1()
        .border_color(rgb(LINE))
        .font_weight(FontWeight::SEMIBOLD)
        .child(title.into())
}

#[cfg(test)]
mod tests;
