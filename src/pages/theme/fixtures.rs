use super::*;

#[derive(Clone)]
pub(super) struct ThemeObject {
    pub(super) symbol: Symbol,
    pub(super) trigger: &'static str,
    pub(super) sector: &'static str,
    pub(super) secondary: &'static str,
    pub(super) method: &'static str,
    pub(super) keyword: &'static str,
    pub(super) related: Vec<Symbol>,
    pub(super) generic: Vec<Symbol>,
    pub(super) counts: [u32; 4],
    pub(super) first: [u32; 4],
    pub(super) second: [u32; 4],
}
pub(super) fn theme_objects() -> Vec<ThemeObject> {
    vec![
        ThemeObject {
            symbol: Symbol::Yinli,
            trigger: "2026-10-14",
            sector: SECTORS[0],
            secondary: "文化传媒",
            method: "连续识别 + 字段识别",
            keyword: "AI 营销",
            related: vec![Symbol::Eastmoney, Symbol::Luxshare],
            generic: vec![Symbol::Smic],
            counts: [4, 6, 8, 9],
            first: [3, 4, 5, 5],
            second: [1, 1, 2, 2],
        },
        ThemeObject {
            symbol: Symbol::Yangguang,
            trigger: "2026-10-13",
            sector: SECTORS[2],
            secondary: "食品饮料",
            method: "字段识别",
            keyword: "消费复苏",
            related: vec![Symbol::Moutai, Symbol::Wuliangye, Symbol::Midea],
            generic: vec![Symbol::Cmb],
            counts: [3, 5, 4, 7],
            first: [2, 3, 2, 4],
            second: [1, 1, 1, 2],
        },
        ThemeObject {
            symbol: Symbol::Seres,
            trigger: "2026-10-12",
            sector: SECTORS[1],
            secondary: "智能驾驶",
            method: "连续识别",
            keyword: "新能源汽车",
            related: vec![Symbol::Byd, Symbol::Catl],
            generic: vec![Symbol::Luxshare],
            counts: [5, 6, 5, 8],
            first: [4, 3, 3, 5],
            second: [1, 2, 1, 2],
        },
        ThemeObject {
            symbol: Symbol::Smic,
            trigger: "2026-10-09",
            sector: SECTORS[3],
            secondary: "国产替代",
            method: "连续识别 + 字段识别",
            keyword: "国产替代",
            related: vec![Symbol::Luxshare, Symbol::Midea],
            generic: vec![Symbol::Eastmoney],
            counts: [3, 4, 6, 6],
            first: [2, 2, 4, 3],
            second: [1, 1, 1, 2],
        },
    ]
}
pub(super) fn valid_date(date: &str) -> bool {
    let bytes = date.as_bytes();
    if bytes.len() != 10
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || bytes
            .iter()
            .enumerate()
            .any(|(i, b)| i != 4 && i != 7 && !b.is_ascii_digit())
    {
        return false;
    }
    let year = date[..4].parse::<u32>().unwrap_or(0);
    let month = date[5..7].parse::<usize>().unwrap_or(0);
    let day = date[8..].parse::<u32>().unwrap_or(0);
    let leap = year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
    let days = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    (1..=12).contains(&month) && day > 0 && day <= days[month - 1]
}
pub(super) fn actual_date(requested: &str) -> Option<&'static str> {
    if !valid_date(requested) {
        return None;
    }
    EVIDENCE_DATES.into_iter().find(|date| *date <= requested)
}

#[cfg(test)]
mod tests;
