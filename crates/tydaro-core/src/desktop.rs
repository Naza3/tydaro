//! Business document contracts and atomic, validated commands.
use crate::{
    market::Symbol,
    research::{ResearchDocument, ResearchRow},
};
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::ops::Deref;
pub const DEMO_DATE: &str = "2026-10-14";
pub const VERSION: u32 = 2;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PageId {
    #[default]
    Market,
    Watch,
    Limit,
    Theme,
    Journal,
}
impl PageId {
    pub const ALL: [Self; 5] = [
        Self::Market,
        Self::Watch,
        Self::Limit,
        Self::Theme,
        Self::Journal,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Market => "行情",
            Self::Watch => "盯盘",
            Self::Limit => "涨停",
            Self::Theme => "题材",
            Self::Journal => "复盘",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SourceContext {
    pub date: String,
    pub sector: String,
    pub symbols: Vec<Symbol>,
    pub reason: String,
    pub origin: PageId,
}
#[derive(Clone, Debug)]
pub enum DesktopEvent {
    Inspect(SourceContext),
    OpenMarket(Symbol),
    AddToWatch(SourceContext),
    CreateDraft(SourceContext),
    OpenTheme(SourceContext),
    CandidateDraft(crate::research::CandidateSet),
    Notice(String),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WatchGroup {
    pub id: String,
    pub name: String,
    pub symbols: Vec<Symbol>,
    pub color: u32,
    pub note: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WatchRule {
    pub id: String,
    pub title: String,
    pub label: String,
    pub unit: String,
    pub threshold: f64,
    pub enabled: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WatchAlert {
    pub id: String,
    pub symbol: Symbol,
    pub rule_id: String,
    pub title: String,
    pub detail: String,
    pub group_name: String,
    pub time: String,
    pub read: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Plan {
    pub id: String,
    pub date: String,
    pub sector: String,
    pub basis: String,
    pub symbols: Vec<Symbol>,
    pub method: String,
    pub entry: String,
    pub avoid: String,
    pub sell_method: String,
    pub exit: String,
    pub hold: String,
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub research_evidence: Option<crate::research::CandidateSet>,
}
impl Plan {
    pub fn validate_complete(&self) -> Result<()> {
        validate_plan_fields(self)?;
        ensure!(!self.sector.trim().is_empty(), "请填写板块或题材");
        let buy = [&self.method, &self.entry, &self.avoid]
            .iter()
            .any(|v| !v.trim().is_empty());
        let sell = [&self.sell_method, &self.exit, &self.hold]
            .iter()
            .any(|v| !v.trim().is_empty());
        ensure!(
            buy || sell,
            "至少填写一侧完整预案，未完成内容可以保存为草稿"
        );
        if buy {
            ensure!(
                !self.method.trim().is_empty() && !self.entry.trim().is_empty(),
                "买入侧需要方式和触发条件"
            );
        }
        if sell {
            ensure!(
                !self.sell_method.trim().is_empty() && !self.exit.trim().is_empty(),
                "卖出侧需要方式和触发条件"
            );
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Draft {
    pub plan: Plan,
    pub editing_id: Option<String>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct JournalDay {
    pub plans: Vec<Plan>,
    pub drafts: Vec<Draft>,
    pub note: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Hotspot {
    pub id: String,
    pub date: String,
    pub sector: String,
    pub heights: String,
    pub created: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BuyRecord {
    pub id: String,
    pub date: String,
    pub stock: String,
    pub sector: String,
    pub method: String,
    pub profit: f64,
    pub note: String,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Journal {
    pub days: BTreeMap<String, JournalDay>,
    pub hotspots: Vec<Hotspot>,
    pub buys: Vec<BuyRecord>,
    pub methods: Vec<String>,
    pub sell_methods: Vec<String>,
}
impl Journal {
    pub fn day_mut(&mut self, date: &str) -> &mut JournalDay {
        self.days.entry(date.to_owned()).or_default()
    }
    /// An edit updates an existing identity, never silently appends a duplicate.
    pub fn save_plan(&mut self, mut plan: Plan, editing_id: Option<&str>) -> Result<()> {
        plan.validate_complete()?;
        if let Some(id) = editing_id {
            let day = self
                .days
                .get_mut(&plan.date)
                .context("原预案日期已不存在")?;
            let previous = day
                .plans
                .iter_mut()
                .find(|p| p.id == id)
                .context("原预案已删除，请另存为新预案")?;
            plan.id = id.to_owned();
            *previous = plan;
        } else {
            ensure!(
                !self
                    .days
                    .values()
                    .any(|day| day.plans.iter().any(|p| p.id == plan.id)),
                "预案 ID 已存在"
            );
            self.day_mut(&plan.date.clone()).plans.push(plan);
        }
        Ok(())
    }
    /// Select the ten latest manual records first, then sort that cohort by return.
    pub fn recent_buys(&self) -> Vec<BuyRecord> {
        let mut recent = self.buys.clone();
        recent.sort_by(|a, b| b.date.cmp(&a.date).then_with(|| a.id.cmp(&b.id)));
        recent.truncate(10);
        recent.sort_by(|a, b| {
            b.profit
                .total_cmp(&a.profit)
                .then_with(|| b.date.cmp(&a.date))
        });
        recent
    }
}

pub fn blank_plan(date: &str, id: String) -> Plan {
    Plan {
        id,
        date: date.into(),
        sector: String::new(),
        basis: String::new(),
        symbols: vec![],
        method: String::new(),
        entry: String::new(),
        avoid: String::new(),
        sell_method: String::new(),
        exit: String::new(),
        hold: String::new(),
        source: "手工编写".into(),
        research_evidence: None,
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DesktopDocument {
    pub version: u32,
    pub next_id: u64,
    pub active_page: PageId,
    pub watch_groups: Vec<WatchGroup>,
    pub monitoring: bool,
    pub muted: bool,
    pub alerts: Vec<WatchAlert>,
    pub rules: Vec<WatchRule>,
    pub journal: Journal,
    #[serde(default)]
    pub research: serde_json::Value,
}

pub fn valid_date(date: &str) -> bool {
    date_valid(date)
}
pub fn date_valid(date: &str) -> bool {
    if date.len() != 10 || !date.is_ascii() || &date[4..5] != "-" || &date[7..8] != "-" {
        return false;
    }
    if !date
        .bytes()
        .enumerate()
        .all(|(i, b)| i == 4 || i == 7 || b.is_ascii_digit())
    {
        return false;
    }
    let (Ok(year), Ok(month), Ok(day)) = (
        date[0..4].parse::<u32>(),
        date[5..7].parse::<u32>(),
        date[8..10].parse::<u32>(),
    ) else {
        return false;
    };
    let leap = year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => 0,
    };
    (1900..=9999).contains(&year) && day > 0 && day <= days
}
fn text_limit(value: &str, limit: usize) -> Result<()> {
    ensure!(value.len() <= limit, "文本长度超出允许范围");
    ensure!(!value.contains('\0'), "文本包含无效字符");
    Ok(())
}
fn identity(value: &str) -> Result<()> {
    ensure!(!value.trim().is_empty(), "记录 ID 不能为空");
    text_limit(value, 128)
}
fn symbols_valid(symbols: &[Symbol]) -> Result<()> {
    let mut ids = HashSet::new();
    ensure!(symbols.iter().all(|s| ids.insert(*s)), "股票列表包含重复项");
    Ok(())
}
fn validate_plan_fields(plan: &Plan) -> Result<()> {
    identity(&plan.id)?;
    ensure!(date_valid(&plan.date), "预案日期无效");
    text_limit(&plan.sector, 512)?;
    for text in [
        &plan.basis,
        &plan.method,
        &plan.entry,
        &plan.avoid,
        &plan.sell_method,
        &plan.exit,
        &plan.hold,
        &plan.source,
    ] {
        text_limit(text, 64 * 1024)?;
    }
    if let Some(snapshot) = &plan.research_evidence {
        snapshot.validate()?;
        ensure!(
            snapshot.applicable_date().as_str() == plan.date,
            "预案与候选适用日不一致"
        );
    }
    symbols_valid(&plan.symbols)
}
pub fn validate(file: &DesktopDocument) -> Result<()> {
    ensure!(
        matches!(file.version, 1 | VERSION),
        "不支持桌面数据版本 {}",
        file.version
    );
    ensure!(
        file.next_id > 0 && file.next_id < u64::MAX / 2,
        "无效 ID 分配器"
    );
    ensure!(file.watch_groups.len() <= 128, "盯盘分组超过上限");
    let mut ids = HashSet::new();
    for group in &file.watch_groups {
        identity(&group.id)?;
        ensure!(ids.insert(&group.id), "分组 ID 重复");
        ensure!(!group.name.trim().is_empty(), "分组名不能为空");
        text_limit(&group.name, 512)?;
        text_limit(&group.note, 16 * 1024)?;
        ensure!(group.color <= 0xffffff, "分组颜色无效");
        symbols_valid(&group.symbols)?;
    }
    ensure!(
        file.rules.len() <= 32 && file.alerts.len() <= 100,
        "规则或异动记录超出上限"
    );
    ids.clear();
    for rule in &file.rules {
        identity(&rule.id)?;
        ensure!(ids.insert(&rule.id), "规则 ID 重复");
        ensure!(
            rule.threshold.is_finite() && rule.threshold >= 0.0 && rule.threshold <= 1_000_000.0,
            "异动阈值无效"
        );
        for text in [&rule.title, &rule.label, &rule.unit] {
            text_limit(text, 512)?;
        }
    }
    ids.clear();
    for alert in &file.alerts {
        identity(&alert.id)?;
        ensure!(ids.insert(&alert.id), "异动 ID 重复");
        for text in [&alert.rule_id, &alert.title, &alert.group_name, &alert.time] {
            text_limit(text, 512)?;
        }
        text_limit(&alert.detail, 4096)?;
    }
    let journal = &file.journal;
    if file.version == 1 {
        ensure!(
            journal.days.values().all(|d| d
                .plans
                .iter()
                .chain(d.drafts.iter().map(|d| &d.plan))
                .all(|p| p.research_evidence.is_none())),
            "schema 1 不支持研究证据快照"
        );
    }
    ensure!(
        journal.days.len() <= 4096 && journal.hotspots.len() <= 5 && journal.buys.len() <= 10_000,
        "复盘记录超出上限"
    );
    let mut plan_ids = HashSet::new();
    let mut draft_ids = HashSet::new();
    let mut total_plans = 0;
    for (date, day) in &journal.days {
        ensure!(date_valid(date), "复盘日期无效");
        text_limit(&day.note, 256 * 1024)?;
        total_plans += day.plans.len() + day.drafts.len();
        ensure!(total_plans <= 20_000, "预案/草稿记录超过上限");
        for plan in &day.plans {
            ensure!(plan_ids.insert(&plan.id), "预案 ID 重复");
            ensure!(&plan.date == date, "预案与日期目录不一致");
            plan.validate_complete()?;
        }
        for draft in &day.drafts {
            ensure!(draft_ids.insert(&draft.plan.id), "草稿 ID 重复");
            ensure!(&draft.plan.date == date, "草稿与日期目录不一致");
            validate_plan_fields(&draft.plan)?;
            if let Some(id) = &draft.editing_id {
                identity(id)?;
            }
        }
    }
    ids.clear();
    for hot in &journal.hotspots {
        identity(&hot.id)?;
        ensure!(ids.insert(&hot.id), "热点 ID 重复");
        ensure!(date_valid(&hot.date), "热点日期无效");
        ensure!(hot.created < u64::MAX / 2, "热点排序序号无效");
        ensure!(!hot.sector.trim().is_empty(), "热点板块不能为空");
        text_limit(&hot.sector, 512)?;
        text_limit(&hot.heights, 16 * 1024)?;
    }
    ids.clear();
    for buy in &journal.buys {
        identity(&buy.id)?;
        ensure!(ids.insert(&buy.id), "收益记录 ID 重复");
        ensure!(
            date_valid(&buy.date) && buy.profit.is_finite(),
            "收益日期或数值无效"
        );
        for text in [&buy.stock, &buy.sector, &buy.method, &buy.note] {
            text_limit(text, 16 * 1024)?;
        }
    }
    ensure!(
        journal.methods.len() <= 128 && journal.sell_methods.len() <= 128,
        "方式记录超过上限"
    );
    for method in journal.methods.iter().chain(&journal.sell_methods) {
        text_limit(method, 512)?;
    }
    let record_ids = file
        .watch_groups
        .iter()
        .map(|v| &v.id)
        .chain(file.alerts.iter().map(|v| &v.id))
        .chain(journal.hotspots.iter().map(|v| &v.id))
        .chain(journal.buys.iter().map(|v| &v.id))
        .chain(
            journal
                .days
                .values()
                .flat_map(|day| day.plans.iter().map(|v| &v.id)),
        )
        .chain(
            journal
                .days
                .values()
                .flat_map(|day| day.drafts.iter().map(|v| &v.plan.id)),
        );
    for id in record_ids {
        if let Some(number) = id
            .rsplit('-')
            .next()
            .and_then(|tail| tail.parse::<u64>().ok())
        {
            ensure!(number < file.next_id, "ID 分配器落后于现有记录");
        }
    }
    Ok(())
}
pub fn seed() -> DesktopDocument {
    use Symbol::*;
    let groups = [
        (
            "media",
            "传媒 · 情绪观察",
            vec![Yinli, Yangguang, Eastmoney],
            0x009b9a,
            "关注核心标的承接与板块跟随",
        ),
        (
            "technology",
            "科技 · 趋势跟踪",
            vec![Smic, Luxshare, Catl],
            0x6686c0,
            "观察放量与均线支撑",
        ),
        (
            "auto",
            "汽车 · 轮动机会",
            vec![Seres, Byd, Midea],
            0xba9466,
            "保留分歧后的确认机会",
        ),
        (
            "consumer",
            "消费 · 防守配置",
            vec![Moutai, Wuliangye, Cmb],
            0x9b83ad,
            "低波动观察，不追涨",
        ),
    ]
    .into_iter()
    .map(|(id, name, symbols, color, note)| WatchGroup {
        id: id.into(),
        name: name.into(),
        symbols,
        color,
        note: note.into(),
    })
    .collect();
    let rules = [
        ("rise", "短线拉升", "3 分钟涨幅达到", "%", 1.5),
        (
            "volume",
            "成交放量",
            "分钟成交量 / 前 5 分钟均量达到",
            "倍",
            2.0,
        ),
        ("near-limit", "接近涨停", "距涨停价格不超过", "%", 1.0),
    ]
    .into_iter()
    .map(|(id, title, label, unit, threshold)| WatchRule {
        id: id.into(),
        title: title.into(),
        label: label.into(),
        unit: unit.into(),
        threshold,
        enabled: true,
    })
    .collect();
    let alerts = [
        (
            "initial-3",
            Yinli,
            "near-limit",
            "接近涨停",
            "距涨停 0.01% ≤ 1.00% · 演示值",
            "传媒 · 情绪观察",
            "14:31:42",
            false,
        ),
        (
            "initial-2",
            Seres,
            "volume",
            "成交放量",
            "分钟量比 2.36 倍 ≥ 2.00 倍 · 演示值",
            "汽车 · 轮动机会",
            "14:30:18",
            false,
        ),
        (
            "initial-1",
            Yangguang,
            "rise",
            "短线拉升",
            "3 分钟涨幅 +1.82% ≥ 1.50% · 演示值",
            "传媒 · 情绪观察",
            "14:28:06",
            true,
        ),
    ]
    .into_iter()
    .map(
        |(id, symbol, rule_id, title, detail, group_name, time, read)| WatchAlert {
            id: id.into(),
            symbol,
            rule_id: rule_id.into(),
            title: title.into(),
            detail: detail.into(),
            group_name: group_name.into(),
            time: time.into(),
            read,
        },
    )
    .collect();
    let mut journal = Journal::default();
    let mut media = blank_plan(DEMO_DATE, "plan-media".into());
    media.sector = "文化传媒".into();
    media.basis = "核心股封板后观察板块跟随，优先确认分歧承接与量能，不根据单一涨幅追涨。".into();
    media.symbols = vec![Yinli];
    media.method = "分歧转一致".into();
    media.entry = "板块保持活跃，核心股回踩均价线后放量承接，再观察确认。".into();
    media.avoid = "板块回落、核心股放量跌破均价线，或开盘直接加速。".into();
    media.sell_method = "冲高分批".into();
    media.exit = "冲高量价背离，或板块整体转弱时按预案处理。".into();
    media.hold = "核心股承接持续，板块梯队保持完整。".into();
    media.source = "题材研究 · 2026-10-14".into();
    let mut tech = blank_plan(DEMO_DATE, "plan-tech".into());
    tech.sector = "半导体".into();
    tech.basis = "趋势观察：以支撑是否有效作为判断依据，等待走势确认。".into();
    tech.symbols = vec![Smic, Luxshare];
    tech.sell_method = "跌破支撑".into();
    tech.exit = "放量跌破前一日低点，反抽未能收回时复核离场条件。".into();
    tech.hold = "缩量调整，仍在关键支撑之上；记录下一次确认点。".into();
    journal.day_mut(DEMO_DATE).plans = vec![media.clone(), tech.clone()];
    journal.day_mut(DEMO_DATE).note = "10:06  传媒板块保持活跃，先观察核心股回封后的承接，不追开盘加速。\n10:24  半导体出现分化，继续跟踪关键支撑。".into();
    media.id = "plan-previous".into();
    media.date = "2026-10-13".into();
    media.sector = "汽车整车".into();
    media.symbols = vec![Seres];
    media.source = "手工编写".into();
    journal.day_mut("2026-10-13").plans.push(media);
    tech.id = "plan-oct12".into();
    tech.date = "2026-10-12".into();
    journal.day_mut("2026-10-12").plans.push(tech);
    journal.hotspots = [
        (
            "hot-media",
            DEMO_DATE,
            "文化传媒",
            "引力传媒 · 4 板\n板块核心承接观察",
            5,
        ),
        (
            "hot-chip",
            DEMO_DATE,
            "半导体",
            "中芯国际 · 趋势观察\n人工记录，不自动查询行情",
            4,
        ),
        ("hot-auto", "2026-10-13", "汽车整车", "赛力斯 · 核心观察", 3),
        (
            "hot-consumer",
            "2026-10-12",
            "消费电子",
            "立讯精密 · 趋势观察",
            2,
        ),
    ]
    .into_iter()
    .map(|(id, date, sector, heights, created)| Hotspot {
        id: id.into(),
        date: date.into(),
        sector: sector.into(),
        heights: heights.into(),
        created,
    })
    .collect();
    journal.buys = [
        ("2026-10-14", "引力传媒", "文化传媒", 12.8),
        ("2026-10-13", "赛力斯", "汽车整车", 8.6),
        ("2026-10-12", "中芯国际", "半导体", 10.0),
        ("2026-10-09", "东方财富", "证券", -2.4),
        ("2026-10-08", "立讯精密", "消费电子", 5.2),
        ("2026-09-30", "阳光乳业", "食品饮料", 3.1),
        ("2026-09-29", "比亚迪", "汽车整车", -1.6),
        ("2026-09-28", "美的集团", "家用电器", 2.7),
        ("2026-09-25", "宁德时代", "电池", 6.4),
        ("2026-09-24", "招商银行", "银行", 0.8),
        ("2026-09-01", "五粮液", "白酒", 25.2),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, (date, stock, sector, profit))| BuyRecord {
        id: format!("buy-{index}"),
        date: date.into(),
        stock: stock.into(),
        sector: sector.into(),
        method: if index % 2 == 0 {
            "分歧转一致"
        } else {
            "趋势回踩"
        }
        .into(),
        profit,
        note: "手工录入的演示记录；收益率不代表账户核算。".into(),
    })
    .collect();
    DesktopDocument {
        version: VERSION,
        next_id: 100,
        active_page: PageId::Market,
        watch_groups: groups,
        monitoring: true,
        muted: true,
        alerts,
        rules,
        journal,
        research: serde_json::Value::Null,
    }
}

/// The wire document is a DTO. Only this aggregate owns the authoritative copy.
/// Read-only dereferencing preserves inexpensive queries without a mutable escape hatch.
#[derive(Clone)]
pub struct Desktop {
    document: DesktopDocument,
    research: Result<Option<ResearchDocument>, String>,
}
impl Deref for Desktop {
    type Target = DesktopDocument;
    fn deref(&self) -> &Self::Target {
        &self.document
    }
}
impl TryFrom<DesktopDocument> for Desktop {
    type Error = anyhow::Error;
    fn try_from(mut document: DesktopDocument) -> Result<Self> {
        validate(&document)?;
        // Read legacy documents without manufacturing provenance. The first successful
        // write uses schema 2, so old applications reject rather than discard evidence.
        document.version = VERSION;
        let research = if document.research.is_null() {
            Ok(None)
        } else {
            ResearchDocument::decode(document.research.clone())
                .map(Some)
                .map_err(|e| e.to_string())
        };
        Ok(Self { document, research })
    }
}
impl Default for Desktop {
    fn default() -> Self {
        Self::try_from(seed()).expect("valid built-in desktop fixture")
    }
}
#[derive(Clone, Debug)]
pub enum Command {
    Navigate(PageId),
    SaveWatchGroup {
        id: Option<String>,
        name: String,
        note: String,
        color: u32,
        symbols: Vec<Symbol>,
    },
    DeleteWatchGroup(String),
    AddToWatch {
        group_id: String,
        symbols: Vec<Symbol>,
    },
    ToggleMonitoring,
    ToggleMuted,
    ReplaceRules(Vec<WatchRule>),
    RecordDemoAlert(WatchAlert),
    ReadAlert(String),
    ReadAllAlerts,
    ClearReadAlerts,
    CreateDraft {
        context: SourceContext,
        date: String,
    },
    NewDraft {
        date: String,
    },
    EditPlan {
        date: String,
        id: String,
    },
    UpsertDraft(Draft),
    SavePlanDraft(Draft),
    SetNote {
        date: String,
        text: String,
    },
    DeletePlan {
        date: String,
        id: String,
    },
    DeleteDraft {
        date: String,
        id: String,
    },
    SaveHotspot {
        id: Option<String>,
        date: String,
        sector: String,
        heights: String,
    },
    SaveBuy {
        id: Option<String>,
        date: String,
        stock: String,
        sector: String,
        method: String,
        profit: f64,
        note: String,
    },
    DeleteHotspot(String),
    DeleteBuy(String),
    SetMethods {
        buy: Vec<String>,
        sell: Vec<String>,
    },
    ReplaceResearchRows(Vec<ResearchRow>),
    CreateCandidateDraft(crate::research::CandidateSet),
}
#[derive(Clone, Debug)]
pub enum CommandResult {
    None,
    Id(String),
    Count(usize),
    Draft(Box<Draft>),
}
impl CommandResult {
    pub fn into_id(self) -> Result<String> {
        match self {
            Self::Id(id) => Ok(id),
            _ => bail!("命令未返回记录 ID"),
        }
    }
    pub fn into_count(self) -> Result<usize> {
        match self {
            Self::Count(n) => Ok(n),
            _ => bail!("命令未返回数量"),
        }
    }
    pub fn into_draft(self) -> Result<Draft> {
        match self {
            Self::Draft(v) => Ok(*v),
            _ => bail!("命令未返回草稿"),
        }
    }
}
impl Desktop {
    pub fn snapshot(&self) -> DesktopDocument {
        self.document.clone()
    }
    pub fn research_document(&self) -> Result<Option<&ResearchDocument>, &str> {
        self.research
            .as_ref()
            .map(|v| v.as_ref())
            .map_err(String::as_str)
    }
    pub fn unread_count(&self) -> usize {
        self.alerts.iter().filter(|v| !v.read).count()
    }
    /// Validate on a candidate copy; failures cannot mutate IDs, records or provenance.
    pub fn execute(&mut self, command: Command) -> Result<CommandResult> {
        let mut candidate = self.clone();
        let result = candidate.apply(command)?;
        validate(&candidate.document)?;
        *self = candidate;
        Ok(result)
    }
    fn new_id(&mut self, prefix: &str) -> String {
        let id = self.document.next_id;
        self.document.next_id = self.document.next_id.saturating_add(1);
        format!("{prefix}-{id}")
    }
    fn apply(&mut self, command: Command) -> Result<CommandResult> {
        use CommandResult as Out;
        match command {
            Command::Navigate(page) => self.document.active_page = page,
            Command::SaveWatchGroup {
                id,
                name,
                note,
                color,
                symbols,
            } => {
                let name = name.trim().to_owned();
                let note = note.trim().to_owned();
                ensure!(
                    !name.is_empty()
                        && name.chars().count() <= 24
                        && !name.chars().any(char::is_control),
                    "分组名称需为 1–24 个字符，不能包含控制字符。"
                );
                ensure!(note.chars().count() <= 160, "观察备注最多 160 个字符。");
                ensure!(
                    !self
                        .watch_groups
                        .iter()
                        .any(|g| Some(&g.id) != id.as_ref() && g.name == name),
                    "已有同名分组，请使用不同的名称。"
                );
                if let Some(id) = &id {
                    ensure!(
                        self.watch_groups.iter().any(|g| &g.id == id),
                        "原分组已删除"
                    );
                } else {
                    ensure!(self.watch_groups.len() < 128, "最多支持 128 个盯盘分组");
                }
                let id = id.unwrap_or_else(|| self.new_id("watch"));
                let group = WatchGroup {
                    id: id.clone(),
                    name,
                    note,
                    color,
                    symbols,
                };
                if let Some(previous) = self.document.watch_groups.iter_mut().find(|g| g.id == id) {
                    *previous = group;
                } else {
                    self.document.watch_groups.push(group);
                }
                return Ok(Out::Id(id));
            }
            Command::DeleteWatchGroup(id) => self.document.watch_groups.retain(|v| v.id != id),
            Command::AddToWatch { group_id, symbols } => {
                let group = self
                    .document
                    .watch_groups
                    .iter_mut()
                    .find(|g| g.id == group_id)
                    .context("目标分组已不存在")?;
                let before = group.symbols.len();
                for symbol in symbols {
                    if !group.symbols.contains(&symbol) {
                        group.symbols.push(symbol);
                    }
                }
                return Ok(Out::Count(group.symbols.len() - before));
            }
            Command::ToggleMonitoring => self.document.monitoring = !self.monitoring,
            Command::ToggleMuted => self.document.muted = !self.muted,
            Command::ReplaceRules(rules) => {
                ensure!(
                    rules.iter().all(|r| r.threshold.is_finite()
                        && r.threshold > 0.
                        && r.threshold <= 100.),
                    "规则阈值请输入大于 0、且不超过 100 的数值。"
                );
                self.document.rules = rules;
            }
            Command::RecordDemoAlert(mut alert) => {
                ensure!(self.monitoring, "监控已暂停");
                ensure!(
                    self.rules
                        .iter()
                        .any(|r| r.id == alert.rule_id && r.enabled),
                    "演示规则未启用"
                );
                alert.id = self.new_id("alert");
                self.document.alerts.insert(0, alert);
                self.document.alerts.truncate(100);
            }
            Command::ReadAlert(id) => {
                if let Some(v) = self.document.alerts.iter_mut().find(|v| v.id == id) {
                    v.read = true;
                }
            }
            Command::ReadAllAlerts => {
                for v in &mut self.document.alerts {
                    v.read = true;
                }
            }
            Command::ClearReadAlerts => self.document.alerts.retain(|v| !v.read),
            Command::CreateDraft { context, date } => {
                ensure!(date_valid(&context.date), "资料日期无效");
                let id = self.new_id("draft");
                let mut plan = blank_plan(&date, id.clone());
                plan.sector = context.sector.clone();
                plan.basis = context.reason;
                for s in context.symbols {
                    if !plan.symbols.contains(&s) {
                        plan.symbols.push(s);
                    }
                }
                plan.source = format!(
                    "{} · {} · {}",
                    context.origin.label(),
                    context.date,
                    context.sector
                );
                self.document.journal.day_mut(&date).drafts.push(Draft {
                    plan,
                    editing_id: None,
                });
                return Ok(Out::Id(id));
            }
            Command::NewDraft { date } => {
                let id = self.new_id("draft");
                self.document.journal.day_mut(&date).drafts.push(Draft {
                    plan: blank_plan(&date, id.clone()),
                    editing_id: None,
                });
                return Ok(Out::Id(id));
            }
            Command::EditPlan { date, id } => {
                let day = self.journal.days.get(&date).context("原预案日期已不存在")?;
                if let Some(existing) = day
                    .drafts
                    .iter()
                    .find(|d| d.editing_id.as_deref() == Some(&id))
                {
                    return Ok(Out::Draft(Box::new(existing.clone())));
                }
                let mut plan = day
                    .plans
                    .iter()
                    .find(|p| p.id == id)
                    .context("原预案已删除")?
                    .clone();
                plan.id = self.new_id("draft");
                let draft = Draft {
                    plan,
                    editing_id: Some(id),
                };
                self.document
                    .journal
                    .day_mut(&date)
                    .drafts
                    .push(draft.clone());
                return Ok(Out::Draft(Box::new(draft)));
            }
            Command::UpsertDraft(draft) => {
                let day = self.document.journal.day_mut(&draft.plan.date);
                if let Some(previous) = day.drafts.iter_mut().find(|d| d.plan.id == draft.plan.id) {
                    *previous = draft;
                } else {
                    day.drafts.push(draft);
                }
            }
            Command::SavePlanDraft(draft) => {
                let date = draft.plan.date.clone();
                let id = draft.plan.id.clone();
                self.document
                    .journal
                    .save_plan(draft.plan, draft.editing_id.as_deref())?;
                self.document
                    .journal
                    .day_mut(&date)
                    .drafts
                    .retain(|d| d.plan.id != id);
            }
            Command::SetNote { date, text } => self.document.journal.day_mut(&date).note = text,
            Command::DeletePlan { date, id } => {
                if let Some(day) = self.document.journal.days.get_mut(&date) {
                    day.plans.retain(|p| p.id != id);
                    day.drafts.retain(|d| d.editing_id.as_deref() != Some(&id));
                }
            }
            Command::DeleteDraft { date, id } => {
                if let Some(day) = self.document.journal.days.get_mut(&date) {
                    day.drafts.retain(|d| d.plan.id != id);
                }
            }
            Command::SaveHotspot {
                id,
                date,
                sector,
                heights,
            } => {
                if let Some(id) = &id {
                    ensure!(
                        self.journal.hotspots.iter().any(|h| &h.id == id),
                        "原热点已删除"
                    );
                } else {
                    ensure!(
                        self.journal.hotspots.len() < 5,
                        "人工热点最多 5 个，请先移除一个"
                    );
                }
                let id = id.unwrap_or_else(|| self.new_id("journal-record"));
                let created = self
                    .journal
                    .hotspots
                    .iter()
                    .map(|h| h.created)
                    .max()
                    .unwrap_or(0)
                    .saturating_add(1);
                let item = Hotspot {
                    id: id.clone(),
                    date,
                    sector,
                    heights,
                    created,
                };
                if let Some(p) = self
                    .document
                    .journal
                    .hotspots
                    .iter_mut()
                    .find(|h| h.id == id)
                {
                    *p = item;
                } else {
                    self.document.journal.hotspots.push(item);
                }
                return Ok(Out::Id(id));
            }
            Command::SaveBuy {
                id,
                date,
                stock,
                sector,
                method,
                profit,
                note,
            } => {
                ensure!(!stock.trim().is_empty(), "请填写股票名称");
                if let Some(id) = &id {
                    ensure!(
                        self.journal.buys.iter().any(|b| &b.id == id),
                        "原收益记录已删除"
                    );
                }
                let id = id.unwrap_or_else(|| self.new_id("journal-record"));
                let item = BuyRecord {
                    id: id.clone(),
                    date,
                    stock,
                    sector,
                    method,
                    profit,
                    note,
                };
                if let Some(p) = self.document.journal.buys.iter_mut().find(|b| b.id == id) {
                    *p = item;
                } else {
                    self.document.journal.buys.push(item);
                }
                return Ok(Out::Id(id));
            }
            Command::DeleteHotspot(id) => self.document.journal.hotspots.retain(|h| h.id != id),
            Command::DeleteBuy(id) => self.document.journal.buys.retain(|b| b.id != id),
            Command::SetMethods { buy, sell } => {
                for methods in [&buy, &sell] {
                    let mut seen = HashSet::new();
                    ensure!(
                        methods.len() <= 128
                            && methods.iter().all(|m| !m.trim().is_empty()
                                && m.len() <= 512
                                && !m.chars().any(char::is_control)
                                && seen.insert(m)),
                        "方法词库包含重复或无效内容"
                    );
                }
                self.document.journal.methods = buy;
                self.document.journal.sell_methods = sell;
            }
            Command::CreateCandidateDraft(set) => {
                let id = self.new_id("draft");
                let draft = set.draft(id.clone())?;
                self.document
                    .journal
                    .day_mut(&draft.plan.date)
                    .drafts
                    .push(draft);
                return Ok(Out::Id(id));
            }
            Command::ReplaceResearchRows(rows) => {
                let mut doc = self
                    .research
                    .as_ref()
                    .map_err(|e| anyhow::anyhow!("研究原文已保护：{e}"))?
                    .clone()
                    .unwrap_or_default();
                doc.rows = rows;
                doc.validate()?;
                self.document.research = serde_json::to_value(&doc)?;
                self.research = Ok(Some(doc));
            }
        }
        Ok(Out::None)
    }
}

#[cfg(test)]
mod command_tests {
    use super::*;
    #[test]
    fn legacy_document_migrates_explicitly_and_future_schema_is_rejected() {
        let mut legacy = seed();
        legacy.version = 1;
        let original = legacy.journal.days[DEMO_DATE].plans[0].source.clone();
        let mut state = Desktop::try_from(legacy).unwrap();
        assert_eq!(state.version, 2);
        assert_eq!(state.journal.days[DEMO_DATE].plans[0].source, original);
        assert!(
            state.journal.days[DEMO_DATE].plans[0]
                .research_evidence
                .is_none()
        );
        let set = crate::research::demonstration_candidates().unwrap();
        state.execute(Command::CreateCandidateDraft(set)).unwrap();
        let snapshot = state.snapshot();
        assert_eq!(snapshot.version, 2);
        let restored = Desktop::try_from(
            serde_json::from_slice::<DesktopDocument>(&serde_json::to_vec(&snapshot).unwrap())
                .unwrap(),
        )
        .unwrap();
        assert!(
            restored.journal.days["2026-10-15"].drafts[0]
                .plan
                .research_evidence
                .is_some()
        );
        let mut unsupported = snapshot;
        unsupported.version = 999;
        assert!(Desktop::try_from(unsupported).is_err());
    }
    #[test]
    fn rejected_commands_are_atomic_and_cannot_bypass_ui_rules() {
        let mut state = Desktop::default();
        let before = serde_json::to_value(state.snapshot()).unwrap();
        assert!(
            state
                .execute(Command::SaveWatchGroup {
                    id: None,
                    name: "x".repeat(25),
                    note: String::new(),
                    color: 0,
                    symbols: vec![]
                })
                .is_err()
        );
        let mut rules = state.rules.clone();
        rules[0].threshold = 0.;
        assert!(state.execute(Command::ReplaceRules(rules)).is_err());
        assert!(
            state
                .execute(Command::SetNote {
                    date: "2026-02-30".into(),
                    text: "invalid".into()
                })
                .is_err()
        );
        assert_eq!(serde_json::to_value(state.snapshot()).unwrap(), before);
    }
    #[test]
    fn commands_preserve_legacy_serialization_and_independent_drafts() {
        let doc = seed();
        let before = serde_json::to_value(&doc).unwrap();
        let mut state = Desktop::try_from(doc).unwrap();
        assert_eq!(serde_json::to_value(state.snapshot()).unwrap(), before);
        let first = state
            .execute(Command::NewDraft {
                date: DEMO_DATE.into(),
            })
            .unwrap()
            .into_id()
            .unwrap();
        let second = state
            .execute(Command::NewDraft {
                date: DEMO_DATE.into(),
            })
            .unwrap()
            .into_id()
            .unwrap();
        assert_ne!(first, second);
        assert_eq!(state.journal.days[DEMO_DATE].drafts.len(), 2);
    }
    #[test]
    fn malformed_research_is_preserved_and_only_research_edits_are_blocked() {
        let mut doc = seed();
        doc.research = serde_json::json!({"rows":"broken","future":17});
        let mut state = Desktop::try_from(doc).unwrap();
        assert!(state.research_document().is_err());
        assert!(state.execute(Command::ReplaceResearchRows(vec![])).is_err());
        state.execute(Command::ToggleMuted).unwrap();
        assert_eq!(state.snapshot().research["future"], 17);
    }
    #[test]
    fn typed_research_preserves_unknown_legacy_fields() {
        let mut doc = seed();
        doc.research = serde_json::json!({"rows":[],"edited":true});
        let mut state = Desktop::try_from(doc).unwrap();
        state.execute(Command::ReplaceResearchRows(vec![])).unwrap();
        assert_eq!(state.snapshot().research["edited"], true);
    }
}

#[cfg(test)]
mod legacy_business_tests {
    use super::*;
    fn complete_sell(date: &str, id: &str) -> Plan {
        let mut plan = blank_plan(date, id.into());
        plan.sector = "测试板块".into();
        plan.sell_method = "跌破支撑".into();
        plan.exit = "收盘低于前日低点".into();
        plan
    }

    #[test]
    fn dates_reject_impossible_or_non_iso_days() {
        for date in ["2024-02-29", "2026-10-14", "2000-02-29"] {
            assert!(date_valid(date));
        }
        for date in [
            "2026-02-29",
            "1900-02-29",
            "2026-13-01",
            "2026-04-31",
            "2026-00-00",
            "2026-+1-01",
            "2026-1-01",
            "2026年10月14日",
        ] {
            assert!(!date_valid(date), "{date}");
        }
    }

    #[test]
    fn sell_only_plan_is_complete_but_partial_buy_side_stays_a_draft() {
        let mut plan = complete_sell(DEMO_DATE, "new-plan");
        assert!(plan.validate_complete().is_ok());
        plan.avoid = "不追高".into();
        assert!(plan.validate_complete().is_err());
        plan.method = "回踩确认".into();
        plan.entry = "量能恢复后确认".into();
        assert!(plan.validate_complete().is_ok());
        plan.sell_method.clear();
        assert!(plan.validate_complete().is_err());
    }

    #[test]
    fn editing_updates_identity_and_missing_target_never_appends() {
        let mut journal = Journal::default();
        journal
            .save_plan(complete_sell(DEMO_DATE, "original"), None)
            .unwrap();
        let mut edited = complete_sell(DEMO_DATE, "editor-draft");
        edited.exit = "新的触发条件".into();
        journal.save_plan(edited.clone(), Some("original")).unwrap();
        let day = &journal.days[DEMO_DATE];
        assert_eq!(day.plans.len(), 1);
        assert_eq!(day.plans[0].id, "original");
        assert_eq!(day.plans[0].exit, "新的触发条件");
        assert!(journal.save_plan(edited.clone(), Some("deleted")).is_err());
        edited.date = "2026-10-13".into();
        assert!(journal.save_plan(edited, Some("original")).is_err());
        assert_eq!(journal.days.len(), 1);
    }

    #[test]
    fn references_create_independent_drafts_without_touching_existing_plans() {
        let mut state = Desktop::default();
        let before = state.journal.days[DEMO_DATE].plans.len();
        let context = SourceContext {
            date: "2026-10-13".into(),
            sector: "文化传媒".into(),
            symbols: vec![Symbol::Yinli, Symbol::Yinli],
            reason: "资料依据".into(),
            origin: PageId::Theme,
        };
        let first = state
            .execute(Command::CreateDraft {
                context: context.clone(),
                date: DEMO_DATE.into(),
            })
            .unwrap()
            .into_id()
            .unwrap();
        let second = state
            .execute(Command::CreateDraft {
                context: context.clone(),
                date: DEMO_DATE.into(),
            })
            .unwrap()
            .into_id()
            .unwrap();
        assert_ne!(first, second);
        let day = &state.journal.days[DEMO_DATE];
        assert_eq!(day.plans.len(), before);
        assert_eq!(day.drafts.len(), 2);
        assert_eq!(day.drafts[0].plan.id, first);
        assert_eq!(day.drafts[0].plan.symbols, vec![Symbol::Yinli]);
        assert!(day.drafts[0].plan.source.contains("2026-10-13"));
        assert_eq!(day.drafts[0].plan.date, DEMO_DATE);
        assert!(day.drafts[0].plan.validate_complete().is_err());
        validate(&state.snapshot()).unwrap(); // Partial sides are valid stored drafts.
    }

    #[test]
    fn watch_append_is_deduplicated_and_groups_remain_independent() {
        let mut state = Desktop::default();
        let other = state.watch_groups[1].symbols.clone();
        assert_eq!(
            state
                .execute(Command::AddToWatch {
                    group_id: "media".into(),
                    symbols: vec![Symbol::Yinli, Symbol::Catl, Symbol::Catl]
                })
                .unwrap()
                .into_count()
                .unwrap(),
            1
        );
        assert_eq!(state.watch_groups[0].symbols.len(), 4);
        assert_eq!(state.watch_groups[1].symbols, other);
        assert!(
            state
                .execute(Command::AddToWatch {
                    group_id: "deleted".into(),
                    symbols: vec![Symbol::Yinli]
                })
                .is_err()
        );
    }

    #[test]
    fn recent_returns_select_dates_before_sorting_profit() {
        let journal = seed().journal;
        let recent = journal.recent_buys();
        assert_eq!(recent.len(), 10);
        assert_eq!(recent[0].stock, "引力传媒");
        assert!(!recent.iter().any(|buy| buy.date == "2026-09-01"));
        assert!(recent.iter().any(|buy| buy.profit == 10.0));
        assert_eq!(recent.iter().filter(|buy| buy.profit > 10.0).count(), 1);
    }
}
