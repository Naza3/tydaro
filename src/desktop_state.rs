//! Shared business data for the five-page desktop. Layouts stay in workspace.json;
//! this versioned store contains only groups, manual research and journal data.
use std::{
    collections::{BTreeMap, HashSet},
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

use anyhow::{Context, Result, bail, ensure};
use gpui_kit::{App, Global};
use serde::{Deserialize, Serialize};

use crate::{market::Symbol, storage};

pub const DEMO_DATE: &str = "2026-10-14";
const VERSION: u32 = 1;
const MAX_BYTES: u64 = 8 * 1024 * 1024;
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(1);

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
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DesktopFile {
    version: u32,
    next_id: u64,
    active_page: PageId,
    watch_groups: Vec<WatchGroup>,
    monitoring: bool,
    muted: bool,
    alerts: Vec<WatchAlert>,
    rules: Vec<WatchRule>,
    journal: Journal,
    #[serde(default)]
    research: serde_json::Value,
}

pub struct DesktopState {
    pub active_page: PageId,
    pub watch_groups: Vec<WatchGroup>,
    pub monitoring: bool,
    pub muted: bool,
    pub alerts: Vec<WatchAlert>,
    pub rules: Vec<WatchRule>,
    pub journal: Journal,
    pub research: serde_json::Value,
    pub save_status: String,
    next_id: u64,
    path: Option<PathBuf>,
    baseline: Option<Vec<u8>>,
    blocked: bool,
    revision: u64,
    dirty: bool,
}
impl Global for DesktopState {}
impl DesktopState {
    pub fn new_id(&mut self, prefix: &str) -> String {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        format!("{prefix}-{id}")
    }
    pub fn unread_count(&self) -> usize {
        self.alerts.iter().filter(|alert| !alert.read).count()
    }
    pub fn add_to_watch(&mut self, group_id: &str, symbols: &[Symbol]) -> Result<usize> {
        let group = self
            .watch_groups
            .iter_mut()
            .find(|group| group.id == group_id)
            .context("目标分组已不存在")?;
        let mut added = 0;
        for symbol in symbols {
            if !group.symbols.contains(symbol) {
                group.symbols.push(*symbol);
                added += 1;
            }
        }
        Ok(added)
    }
    /// Always appends a fresh draft; a cross-page reference never edits a plan.
    pub fn create_draft(&mut self, context: &SourceContext, date: &str) -> String {
        let id = self.new_id("draft");
        let mut plan = blank_plan(date, id.clone());
        plan.sector = context.sector.clone();
        plan.basis = context.reason.clone();
        for symbol in &context.symbols {
            if !plan.symbols.contains(symbol) {
                plan.symbols.push(*symbol);
            }
        }
        plan.source = format!(
            "{} · {} · {}",
            context.origin.label(),
            context.date,
            context.sector
        );
        self.journal.day_mut(date).drafts.push(Draft {
            plan,
            editing_id: None,
        });
        id
    }
    fn snapshot(&self) -> DesktopFile {
        DesktopFile {
            version: VERSION,
            next_id: self.next_id,
            active_page: self.active_page,
            watch_groups: self.watch_groups.clone(),
            monitoring: self.monitoring,
            muted: self.muted,
            alerts: self.alerts.clone(),
            rules: self.rules.clone(),
            journal: self.journal.clone(),
            research: self.research.clone(),
        }
    }
    fn from_file(file: DesktopFile) -> Self {
        Self {
            active_page: file.active_page,
            watch_groups: file.watch_groups,
            monitoring: file.monitoring,
            muted: file.muted,
            alerts: file.alerts,
            rules: file.rules,
            journal: file.journal,
            research: file.research,
            next_id: file.next_id,
            path: None,
            baseline: None,
            blocked: false,
            revision: 0,
            dirty: false,
            save_status: "本地演示数据".into(),
        }
    }
}

/// `persistent = false` is used by smoke tests and never opens a user's profile.
pub fn init(persistent: bool, cx: &mut App) {
    cx.set_global(load_state(
        persistent.then(|| storage::data_dir().join("desktop.json")),
    ));
}

fn load_state(path: Option<PathBuf>) -> DesktopState {
    let mut state = DesktopState::from_file(seed());
    if let Some(path) = path {
        match read_bytes(&path) {
            Ok(bytes) => match decode(&bytes) {
                Ok(saved) => {
                    state = DesktopState::from_file(saved);
                    state.baseline = Some(bytes);
                    state.save_status = "已恢复本地数据".into();
                }
                Err(error) => {
                    state.blocked = true;
                    state.save_status = format!("读取失败，原文件已保护；修改仅在内存：{error}");
                }
            },
            Err(error) if is_not_found(&error) => {
                state.save_status = "新工作区 · 修改后自动保存".into()
            }
            Err(error) => {
                state.blocked = true;
                state.save_status = format!("读取失败，原文件已保护；修改仅在内存：{error}");
            }
        }
        state.path = Some(path);
    } else {
        state.save_status = "测试会话 · 不写入磁盘".into();
    }
    state
}

/// Change shared business data and debounce durable writes. All views refresh,
/// including journal reading windows; their input/focus state remains local.
pub fn update<R>(cx: &mut App, mutate: impl FnOnce(&mut DesktopState) -> R) -> R {
    let state = cx.global_mut::<DesktopState>();
    let result = mutate(state);
    state.revision = state.revision.wrapping_add(1);
    state.dirty = true;
    let revision = state.revision;
    if state.path.is_some() && !state.blocked {
        state.save_status = "正在保存…".into();
    }
    cx.refresh_windows();
    cx.spawn(async move |cx| {
        cx.background_executor()
            .timer(Duration::from_millis(700))
            .await;
        cx.update(|cx| {
            if cx.global::<DesktopState>().revision == revision {
                save_now(cx);
            }
        });
    })
    .detach();
    result
}

/// Flush on main-window close as well as on the debounced timer.
pub fn save_now(cx: &mut App) -> bool {
    let saved = persist_state(cx.global_mut::<DesktopState>());
    cx.refresh_windows();
    saved
}

fn persist_state(state: &mut DesktopState) -> bool {
    if !state.dirty {
        return !state.blocked;
    }
    let Some(path) = &state.path else {
        state.dirty = false;
        return true;
    };
    if state.blocked {
        return false;
    }
    match write_file(path, &state.snapshot(), state.baseline.as_deref()) {
        Ok(bytes) => {
            state.baseline = Some(bytes);
            state.dirty = false;
            state.save_status = "已保存到本机".into();
        }
        Err(error) => {
            state.save_status = format!("保存失败，修改仅在内存：{error}");
            return false;
        }
    }
    true
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
    symbols_valid(&plan.symbols)
}
fn validate(file: &DesktopFile) -> Result<()> {
    ensure!(
        file.version == VERSION,
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
fn decode(bytes: &[u8]) -> Result<DesktopFile> {
    let file = serde_json::from_slice(bytes).context("桌面数据 JSON 无效")?;
    validate(&file)?;
    Ok(file)
}
fn read_bytes(path: &Path) -> Result<Vec<u8>> {
    let file = File::open(path).context("无法读取桌面数据文件")?;
    let mut bytes = Vec::new();
    file.take(MAX_BYTES + 1).read_to_end(&mut bytes)?;
    ensure!(bytes.len() as u64 <= MAX_BYTES, "桌面数据文件超过 8 MiB");
    Ok(bytes)
}
fn is_not_found(error: &anyhow::Error) -> bool {
    error
        .downcast_ref::<std::io::Error>()
        .is_some_and(|e| e.kind() == std::io::ErrorKind::NotFound)
}
fn suffix(path: &Path, tail: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(tail);
    PathBuf::from(name)
}
struct PendingFile {
    path: PathBuf,
}
impl PendingFile {
    fn create(path: &Path, bytes: &[u8]) -> Result<Self> {
        for _ in 0..128 {
            let n = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let candidate = suffix(path, &format!(".tmp-{}-{n}", std::process::id()));
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&candidate)
            {
                Ok(mut file) => {
                    let pending = Self { path: candidate };
                    file.write_all(bytes)?;
                    file.sync_all()?;
                    drop(file); // Windows replacement must not retain an open temporary handle.
                    return Ok(pending);
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error).context("无法创建桌面数据临时文件"),
            }
        }
        bail!("无法分配桌面数据临时文件")
    }
    fn replace(self, path: &Path) -> Result<()> {
        fs::rename(&self.path, path).context("无法原子替换桌面数据")?;
        Ok(())
    }
}
impl Drop for PendingFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

/// Refuse unreadable or externally changed predecessors. A valid predecessor is
/// retained byte-for-byte in .bak before the same-directory atomic replacement.
fn write_file(path: &Path, file: &DesktopFile, baseline: Option<&[u8]>) -> Result<Vec<u8>> {
    validate(file)?;
    let bytes = serde_json::to_vec_pretty(file)?;
    ensure!(bytes.len() as u64 <= MAX_BYTES, "桌面数据超过 8 MiB");
    let previous = match read_bytes(path) {
        Ok(bytes) => {
            decode(&bytes).context("现有文件异常，已停止覆盖")?;
            Some(bytes)
        }
        Err(error) if is_not_found(&error) => None,
        Err(error) => return Err(error),
    };
    ensure!(
        previous.as_deref() == baseline,
        "文件已被外部修改，已停止覆盖；请保留当前内容后重新打开"
    );
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(parent)?;
    let pending = PendingFile::create(path, &bytes)?;
    if let Some(previous) = previous {
        let backup = suffix(path, ".bak");
        PendingFile::create(&backup, &previous)?.replace(&backup)?;
    }
    pending.replace(path)?;
    #[cfg(unix)]
    File::open(parent)?.sync_all()?;
    Ok(bytes)
}

fn seed() -> DesktopFile {
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
    DesktopFile {
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

#[cfg(test)]
mod tests {
    use super::*;

    struct TestDirectory(PathBuf);
    impl TestDirectory {
        fn new() -> Self {
            let n = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let path =
                std::env::temp_dir().join(format!("tydaro-desktop-{}-{n}", std::process::id()));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn file(&self) -> PathBuf {
            self.0.join("desktop.json")
        }
    }
    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
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
        let mut state = DesktopState::from_file(seed());
        let before = state.journal.days[DEMO_DATE].plans.len();
        let context = SourceContext {
            date: "2026-10-13".into(),
            sector: "文化传媒".into(),
            symbols: vec![Symbol::Yinli, Symbol::Yinli],
            reason: "资料依据".into(),
            origin: PageId::Theme,
        };
        let first = state.create_draft(&context, DEMO_DATE);
        let second = state.create_draft(&context, DEMO_DATE);
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
        let mut state = DesktopState::from_file(seed());
        let other = state.watch_groups[1].symbols.clone();
        assert_eq!(
            state
                .add_to_watch("media", &[Symbol::Yinli, Symbol::Catl, Symbol::Catl])
                .unwrap(),
            1
        );
        assert_eq!(state.watch_groups[0].symbols.len(), 4);
        assert_eq!(state.watch_groups[1].symbols, other);
        assert!(state.add_to_watch("deleted", &[Symbol::Yinli]).is_err());
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

    #[test]
    fn roundtrip_keeps_independent_data_and_exact_previous_backup() {
        let directory = TestDirectory::new();
        let path = directory.file();
        let mut state = seed();
        let original = write_file(&path, &state, None).unwrap();
        state.monitoring = false;
        state.watch_groups[0].name = "独立观察组".into();
        state.research = serde_json::json!({"rows":[],"edited":true});
        state.next_id = 102;
        state.journal.day_mut("2026-10-15").drafts.push(Draft {
            plan: blank_plan("2026-10-15", "draft-101".into()),
            editing_id: None,
        });
        let saved = write_file(&path, &state, Some(&original)).unwrap();
        assert_eq!(fs::read(suffix(&path, ".bak")).unwrap(), original);
        assert_eq!(fs::read(&path).unwrap(), saved);
        let loaded = decode(&saved).unwrap();
        assert!(!loaded.monitoring);
        assert_eq!(loaded.watch_groups[0].name, "独立观察组");
        assert_eq!(
            loaded.journal.days["2026-10-15"].drafts[0].plan.date,
            "2026-10-15"
        );
        assert_eq!(loaded.research["edited"], true);
        assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 2);
    }

    #[test]
    fn damaged_unknown_or_external_data_is_never_replaced() {
        let directory = TestDirectory::new();
        let path = directory.file();
        let original = write_file(&path, &seed(), None).unwrap();
        for data in [b"{ broken".to_vec(), b"{\"version\":900}".to_vec()] {
            fs::write(&path, &data).unwrap();
            assert!(write_file(&path, &seed(), Some(&original)).is_err());
            assert_eq!(fs::read(&path).unwrap(), data);
            assert!(!suffix(&path, ".bak").exists());
        }
        let mut external = seed();
        external.muted = false;
        let bytes = serde_json::to_vec_pretty(&external).unwrap();
        fs::write(&path, &bytes).unwrap();
        assert!(write_file(&path, &seed(), Some(&original)).is_err());
        assert_eq!(fs::read(&path).unwrap(), bytes);
        fs::remove_file(&path).unwrap();
        assert!(write_file(&path, &seed(), Some(&original)).is_err());
        assert!(!path.exists());
    }

    #[test]
    fn invalid_new_state_leaves_valid_disk_and_backup_unchanged() {
        let directory = TestDirectory::new();
        let path = directory.file();
        let mut file = seed();
        let original = write_file(&path, &file, None).unwrap();
        file.watch_groups[0].symbols.push(Symbol::Yinli);
        assert!(write_file(&path, &file, Some(&original)).is_err());
        assert_eq!(fs::read(&path).unwrap(), original);
        assert!(!suffix(&path, ".bak").exists());
    }

    #[test]
    fn bounded_read_and_write_errors_preserve_current_memory_document() {
        let directory = TestDirectory::new();
        let path = directory.file();
        File::create(&path).unwrap().set_len(MAX_BYTES + 1).unwrap();
        assert!(read_bytes(&path).is_err());
        assert!(write_file(&path, &seed(), None).is_err());
        assert_eq!(fs::metadata(&path).unwrap().len(), MAX_BYTES + 1);
        let blocked = directory.0.join("not-a-directory");
        fs::write(&blocked, b"keep").unwrap();
        assert!(write_file(&blocked.join("desktop.json"), &seed(), None).is_err());
        assert_eq!(fs::read(blocked).unwrap(), b"keep");
    }

    #[test]
    fn loaded_corrupt_file_is_protected_and_save_failures_keep_drafts_in_memory() {
        let directory = TestDirectory::new();
        let path = directory.file();
        fs::write(&path, b"broken original").unwrap();
        let mut state = load_state(Some(path.clone()));
        state.journal.day_mut(DEMO_DATE).note = "new text".into();
        state.dirty = true;
        assert!(!persist_state(&mut state));
        assert!(state.save_status.contains("原文件已保护"));
        assert_eq!(fs::read(&path).unwrap(), b"broken original");
        assert_eq!(state.journal.days[DEMO_DATE].note, "new text");
        fs::remove_file(&path).unwrap();
        let mut state = load_state(Some(path.clone()));
        state.dirty = true;
        assert!(persist_state(&mut state));
        assert_eq!(state.save_status, "已保存到本机");
        fs::write(&path, b"externally damaged").unwrap();
        state.journal.day_mut(DEMO_DATE).note = "unsaved newer text".into();
        state.dirty = true;
        assert!(!persist_state(&mut state));
        assert!(state.save_status.contains("保存失败"));
        assert!(state.dirty);
        assert_eq!(state.journal.days[DEMO_DATE].note, "unsaved newer text");
        assert_eq!(fs::read(path).unwrap(), b"externally damaged");
    }

    #[test]
    fn file_validation_rejects_duplicate_identity_and_cross_date_plan() {
        let mut file = seed();
        validate(&file).unwrap();
        let duplicate = file.watch_groups[0].clone();
        file.watch_groups.push(duplicate);
        assert!(validate(&file).is_err());
        file.watch_groups.pop();
        file.journal.day_mut(DEMO_DATE).plans[0].date = "2026-10-13".into();
        assert!(validate(&file).is_err());
    }
}
