//! Typed legacy research documents. Temporal research rules are independent of views.
use crate::{desktop::date_valid, market::Symbol};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResearchRow {
    pub id: String,
    pub date: String,
    pub sector: String,
    pub symbol: Symbol,
    pub name: String,
    pub board: String,
    pub reason: String,
    pub source: String,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ResearchDocument {
    pub rows: Vec<ResearchRow>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}
impl ResearchDocument {
    pub fn validate(&self) -> Result<()> {
        ensure!(self.rows.len() <= 100_000, "研究资料条数超过上限");
        let mut ids = HashSet::new();
        for row in &self.rows {
            ensure!(date_valid(&row.date), "研究资料日期无效");
            ensure!(
                !row.id.trim().is_empty() && ids.insert(&row.id),
                "研究资料 ID 无效或重复"
            );
            ensure!(!row.sector.trim().is_empty(), "研究资料板块不能为空");
            for value in [
                &row.id,
                &row.sector,
                &row.name,
                &row.board,
                &row.reason,
                &row.source,
            ] {
                ensure!(
                    value.len() <= 64 * 1024 && !value.contains('\0'),
                    "研究资料文本无效"
                );
            }
        }
        Ok(())
    }
    pub fn decode(value: serde_json::Value) -> Result<Self> {
        let document: Self = serde_json::from_value(value)?;
        document.validate()?;
        Ok(document)
    }
}

/// Dates and timestamps are validated at construction AND deserialization.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct TradingDate(String);
impl TradingDate {
    pub fn new(value: impl Into<String>) -> Result<Self> {
        let value = value.into();
        ensure!(date_valid(&value), "无效研究日期");
        Ok(Self(value))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl TryFrom<String> for TradingDate {
    type Error = anyhow::Error;
    fn try_from(value: String) -> Result<Self> {
        Self::new(value)
    }
}
impl From<TradingDate> for String {
    fn from(value: TradingDate) -> String {
        value.0
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(try_from = "TimeDto", into = "TimeDto")]
pub struct ResearchTime {
    date: TradingDate,
    second: u32,
}
#[derive(Clone, Serialize, Deserialize)]
struct TimeDto {
    date: TradingDate,
    second: u32,
}
impl ResearchTime {
    /// A market-local time; all records in one research request share its calendar.
    pub fn new(date: TradingDate, second: u32) -> Result<Self> {
        ensure!(second < 86400, "无效研究时刻");
        Ok(Self { date, second })
    }
    pub fn date(&self) -> &TradingDate {
        &self.date
    }
}
impl TryFrom<TimeDto> for ResearchTime {
    type Error = anyhow::Error;
    fn try_from(v: TimeDto) -> Result<Self> {
        Self::new(v.date, v.second)
    }
}
impl From<ResearchTime> for TimeDto {
    fn from(v: ResearchTime) -> Self {
        Self {
            date: v.date,
            second: v.second,
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ThemeId(String);
impl ThemeId {
    pub fn new(value: impl Into<String>) -> Result<Self> {
        let v = value.into();
        ensure!(
            !v.trim().is_empty() && v.len() <= 128 && !v.chars().any(char::is_control),
            "无效题材 ID"
        );
        Ok(Self(v))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl TryFrom<String> for ThemeId {
    type Error = anyhow::Error;
    fn try_from(v: String) -> Result<Self> {
        Self::new(v)
    }
}
impl From<ThemeId> for String {
    fn from(v: ThemeId) -> String {
        v.0
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum LimitStatus {
    Confirmed { consecutive: u16 },
    NotLimit,
    Suspended,
    Unknown(String),
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LimitObservation {
    pub symbol: Symbol,
    pub date: TradingDate,
    pub known_at: ResearchTime,
    pub revision: u64,
    pub source: String,
    pub status: LimitStatus,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MembershipObservation {
    pub theme: ThemeId,
    pub symbol: Symbol,
    pub date: TradingDate,
    pub known_at: ResearchTime,
    pub revision: u64,
    pub confirmed: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AssessmentObservation {
    pub id: String,
    pub theme: ThemeId,
    pub date: TradingDate,
    pub confirmed_at: ResearchTime,
    pub revision: u64,
    pub strong: bool,
    pub human_confirmed: bool,
    pub reason: String,
    pub evidence_version: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Candidate {
    pub symbol: Symbol,
    pub themes: Vec<ThemeId>,
    pub assessment_ids: Vec<String>,
    pub fact_revision: u64,
    pub fact_source: String,
    pub fact_known_at: ResearchTime,
    pub membership_revisions: Vec<u64>,
    pub assessment_revisions: Vec<u64>,
    pub evidence_versions: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CandidateDecision {
    pub symbol: Symbol,
    pub reason: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(try_from = "CandidateSetDto", into = "CandidateSetDto")]
pub struct CandidateSet {
    source_date: TradingDate,
    applicable_date: TradingDate,
    as_of: ResearchTime,
    rule_version: String,
    candidates: Vec<Candidate>,
    excluded: Vec<CandidateDecision>,
    pending: Vec<CandidateDecision>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CandidateSetDto {
    source_date: TradingDate,
    applicable_date: TradingDate,
    as_of: ResearchTime,
    rule_version: String,
    candidates: Vec<Candidate>,
    excluded: Vec<CandidateDecision>,
    pending: Vec<CandidateDecision>,
}
impl TryFrom<CandidateSetDto> for CandidateSet {
    type Error = anyhow::Error;
    fn try_from(v: CandidateSetDto) -> Result<Self> {
        let result = Self {
            source_date: v.source_date,
            applicable_date: v.applicable_date,
            as_of: v.as_of,
            rule_version: v.rule_version,
            candidates: v.candidates,
            excluded: v.excluded,
            pending: v.pending,
        };
        result.validate()?;
        Ok(result)
    }
}
impl From<CandidateSet> for CandidateSetDto {
    fn from(v: CandidateSet) -> Self {
        Self {
            source_date: v.source_date,
            applicable_date: v.applicable_date,
            as_of: v.as_of,
            rule_version: v.rule_version,
            candidates: v.candidates,
            excluded: v.excluded,
            pending: v.pending,
        }
    }
}
impl CandidateSet {
    pub fn source_date(&self) -> &TradingDate {
        &self.source_date
    }
    pub fn applicable_date(&self) -> &TradingDate {
        &self.applicable_date
    }
    pub fn candidates(&self) -> &[Candidate] {
        &self.candidates
    }
    pub fn pending(&self) -> &[CandidateDecision] {
        &self.pending
    }
    pub fn excluded(&self) -> &[CandidateDecision] {
        &self.excluded
    }
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.applicable_date > self.source_date && self.as_of.date == self.source_date,
            "候选时点与适用日无效"
        );
        ensure!(self.as_of.second >= 15 * 3600, "候选时点早于收盘");
        let valid_text =
            |v: &str| !v.trim().is_empty() && v.len() <= 512 && !v.chars().any(char::is_control);
        ensure!(valid_text(&self.rule_version), "候选缺少规则版本");
        let mut symbols = HashSet::new();
        for c in &self.candidates {
            let mut themes = HashSet::new();
            ensure!(
                symbols.insert(c.symbol)
                    && !c.themes.is_empty()
                    && c.themes.iter().all(|t| themes.insert(t.as_str()))
                    && c.assessment_ids.len() == c.themes.len()
                    && c.evidence_versions.len() == c.themes.len()
                    && c.assessment_ids.iter().all(|v| valid_text(v))
                    && c.evidence_versions.iter().all(|v| valid_text(v))
                    && c.fact_revision > 0
                    && valid_text(&c.fact_source)
                    && c.fact_known_at <= self.as_of
                    && c.fact_known_at.date >= self.source_date
                    && c.membership_revisions.len() == c.themes.len()
                    && c.assessment_revisions.len() == c.themes.len()
                    && c.membership_revisions
                        .iter()
                        .chain(&c.assessment_revisions)
                        .all(|v| *v > 0),
                "候选依据不完整或重复"
            );
        }
        for decision in self.excluded.iter().chain(&self.pending) {
            ensure!(
                symbols.insert(decision.symbol) && valid_text(&decision.reason),
                "候选排除/待核验依据无效或重复"
            );
        }
        Ok(())
    }
    pub fn draft(&self, id: String) -> Result<crate::desktop::Draft> {
        self.validate()?;
        ensure!(!self.candidates.is_empty(), "没有符合条件的首板候选");
        let mut plan = crate::desktop::blank_plan(self.applicable_date.as_str(), id);
        plan.symbols = self.candidates.iter().map(|c| c.symbol).collect();
        let themes = self
            .candidates
            .iter()
            .flat_map(|c| c.themes.iter().map(|t| t.as_str()))
            .collect::<std::collections::BTreeSet<_>>();
        plan.sector = themes.into_iter().collect::<Vec<_>>().join(" / ");
        plan.method = "强势题材二板观察".into();
        plan.basis = format!(
            "{} 收盘首板、截至 {} 秒已人工确认的强势题材；仅候选，次日触发条件待填写。",
            self.source_date.as_str(),
            self.as_of.second
        );
        plan.source = format!(
            "研究候选 · {} · {}",
            self.source_date.as_str(),
            self.rule_version
        );
        // The exact frozen contract accompanies the legacy text fields, not a reconstructed source string.
        plan.research_evidence = Some(self.clone());
        Ok(crate::desktop::Draft {
            plan,
            editing_id: None,
        })
    }
}
/// An append-only evidence store. Queries select only records known at the requested time.
/// It deliberately has no clock, storage, data-source SDK or GUI dependency.
#[derive(Clone, Debug, Default)]
pub struct CandidateResearch {
    facts: Vec<LimitObservation>,
    memberships: Vec<MembershipObservation>,
    assessments: Vec<AssessmentObservation>,
}
impl CandidateResearch {
    pub fn add_fact(&mut self, value: LimitObservation) -> Result<()> {
        ensure!(
            value.revision > 0
                && !value.source.trim().is_empty()
                && value.known_at.date >= value.date,
            "涨停事实来源或时点无效"
        );
        if let LimitStatus::Confirmed { consecutive } = value.status {
            ensure!(consecutive > 0, "已确认板数必须为正");
        }
        ensure!(
            !self.facts.iter().any(|v| v.symbol == value.symbol
                && v.date == value.date
                && v.revision == value.revision),
            "重复事实版本"
        );
        self.facts.push(value);
        Ok(())
    }
    pub fn add_membership(&mut self, value: MembershipObservation) -> Result<()> {
        ensure!(value.revision > 0, "题材归属版本无效");
        ensure!(
            !self.memberships.iter().any(|v| v.theme == value.theme
                && v.symbol == value.symbol
                && v.date == value.date
                && v.revision == value.revision),
            "重复归属版本"
        );
        self.memberships.push(value);
        Ok(())
    }
    pub fn add_assessment(&mut self, value: AssessmentObservation) -> Result<()> {
        ensure!(
            !value.id.trim().is_empty()
                && value.revision > 0
                && !value.evidence_version.trim().is_empty()
                && !value.reason.trim().is_empty()
                && value.confirmed_at.date >= value.date,
            "人工题材判断依据或时点无效"
        );
        ensure!(
            !self.assessments.iter().any(|v| v.id == value.id),
            "重复判断 ID"
        );
        self.assessments.push(value);
        Ok(())
    }
    pub fn generate(
        &self,
        date: TradingDate,
        as_of: ResearchTime,
        calendar: &[TradingDate],
        rule_version: &str,
    ) -> Result<CandidateSet> {
        ensure!(
            as_of.date == date && as_of.second >= 15 * 3600,
            "首板候选需在指定业务日收盘后生成"
        );
        ensure!(!rule_version.trim().is_empty(), "请提供模式规则版本");
        ensure!(
            calendar.windows(2).all(|v| v[0] < v[1]),
            "交易日历必须严格递增且不可重复"
        );
        let index = calendar
            .iter()
            .position(|d| d == &date)
            .ok_or_else(|| anyhow::anyhow!("交易日历缺少源日期"))?;
        let next = calendar
            .get(index + 1)
            .ok_or_else(|| anyhow::anyhow!("交易日历缺少下一交易日"))?
            .clone();
        let mut assessments: BTreeMap<ThemeId, &AssessmentObservation> = BTreeMap::new();
        for value in self
            .assessments
            .iter()
            .filter(|v| v.date == date && v.confirmed_at <= as_of && v.human_confirmed)
        {
            let replace = assessments.get(&value.theme).is_none_or(|old| {
                (&value.confirmed_at, value.revision) > (&old.confirmed_at, old.revision)
            });
            if replace {
                assessments.insert(value.theme.clone(), value);
            }
        }
        let mut member_versions: BTreeMap<(ThemeId, String), &MembershipObservation> =
            BTreeMap::new();
        for value in self
            .memberships
            .iter()
            .filter(|v| v.date == date && v.known_at <= as_of)
        {
            let key = (value.theme.clone(), value.symbol.code().to_owned());
            let replace = member_versions.get(&key).is_none_or(|old| {
                (&value.known_at, value.revision) > (&old.known_at, old.revision)
            });
            if replace {
                member_versions.insert(key, value);
            }
        }
        type MembershipEvidence<'a> = (&'a AssessmentObservation, &'a MembershipObservation);
        let mut grouped: BTreeMap<String, (Symbol, Vec<MembershipEvidence<'_>>)> = BTreeMap::new();
        for value in member_versions.values().filter(|v| v.confirmed) {
            if let Some(assessment) = assessments
                .get(&value.theme)
                .filter(|v| v.strong && v.human_confirmed)
            {
                grouped
                    .entry(value.symbol.code().to_owned())
                    .or_insert_with(|| (value.symbol, Vec::new()))
                    .1
                    .push((assessment, value));
            }
        }
        let mut candidates = Vec::new();
        let mut excluded = Vec::new();
        let mut pending = Vec::new();
        for (symbol, assessments) in grouped.into_values() {
            let fact = self
                .facts
                .iter()
                .filter(|v| v.symbol == symbol && v.date == date && v.known_at <= as_of)
                .max_by(|a, b| (&a.known_at, a.revision).cmp(&(&b.known_at, b.revision)));
            match fact.map(|f| &f.status) {
                Some(LimitStatus::Confirmed { consecutive: 1 }) => {
                    let fact = fact.expect("matched fact");
                    candidates.push(Candidate {
                        symbol,
                        themes: assessments.iter().map(|(a, _)| a.theme.clone()).collect(),
                        assessment_ids: assessments.iter().map(|(a, _)| a.id.clone()).collect(),
                        fact_revision: fact.revision,
                        fact_source: fact.source.clone(),
                        fact_known_at: fact.known_at.clone(),
                        membership_revisions: assessments.iter().map(|(_, m)| m.revision).collect(),
                        assessment_revisions: assessments.iter().map(|(a, _)| a.revision).collect(),
                        evidence_versions: assessments
                            .iter()
                            .map(|(a, _)| a.evidence_version.clone())
                            .collect(),
                    });
                }
                Some(LimitStatus::Unknown(reason)) => pending.push(CandidateDecision {
                    symbol,
                    reason: format!("板数待核验：{reason}"),
                }),
                None => pending.push(CandidateDecision {
                    symbol,
                    reason: "截至所选时刻缺少收盘事实".into(),
                }),
                _ => excluded.push(CandidateDecision {
                    symbol,
                    reason: "当日不是已确认首板涨停".into(),
                }),
            }
        }
        let result = CandidateSet {
            source_date: date,
            applicable_date: next,
            as_of,
            rule_version: rule_version.into(),
            candidates,
            excluded,
            pending,
        };
        result.validate()?;
        Ok(result)
    }
}

/// A deterministic end-to-end fixture for the explicit demonstration action in the theme page.
/// This is separate from the legacy editable limit list and does not claim to derive its facts.
pub fn demonstration_candidates() -> Result<CandidateSet> {
    let date = TradingDate::new("2026-10-14")?;
    let next = TradingDate::new("2026-10-15")?;
    let time = ResearchTime::new(date.clone(), 16 * 3600)?;
    let theme = ThemeId::new("demo-ai-applications")?;
    let mut research = CandidateResearch::default();
    research.add_fact(LimitObservation {
        symbol: Symbol::Eastmoney,
        date: date.clone(),
        known_at: time.clone(),
        revision: 1,
        source: "冻结模拟收盘事实".into(),
        status: LimitStatus::Confirmed { consecutive: 1 },
    })?;
    research.add_membership(MembershipObservation {
        theme: theme.clone(),
        symbol: Symbol::Eastmoney,
        date: date.clone(),
        known_at: time.clone(),
        revision: 1,
        confirmed: true,
    })?;
    research.add_assessment(AssessmentObservation {
        id: "demo-assessment-1".into(),
        theme,
        date: date.clone(),
        confirmed_at: time.clone(),
        revision: 1,
        strong: true,
        human_confirmed: true,
        reason: "演示用户确认；不是对当前题材的自动认定".into(),
        evidence_version: "demo-evidence-1".into(),
    })?;
    research.generate(
        date.clone(),
        time,
        &[date, next],
        "strong-theme-first-limit-v1",
    )
}

#[cfg(test)]
mod temporal_tests {
    use super::*;
    fn day(v: &str) -> TradingDate {
        TradingDate::new(v).unwrap()
    }
    fn time(v: &str, h: u32) -> ResearchTime {
        ResearchTime::new(day(v), h * 3600).unwrap()
    }
    fn fixture() -> CandidateResearch {
        let mut r = CandidateResearch::default();
        for (id, symbol) in [
            ("a", Symbol::Yinli),
            ("b", Symbol::Yinli),
            ("a", Symbol::Seres),
            ("c", Symbol::Catl),
        ] {
            r.add_membership(MembershipObservation {
                theme: ThemeId::new(id).unwrap(),
                symbol,
                date: day("2026-10-16"),
                known_at: time("2026-10-16", 16),
                revision: 1,
                confirmed: true,
            })
            .unwrap();
        }
        for id in ["a", "b", "c"] {
            r.add_assessment(AssessmentObservation {
                id: format!("assessment-{id}"),
                theme: ThemeId::new(id).unwrap(),
                date: day("2026-10-16"),
                confirmed_at: time("2026-10-16", 16),
                revision: 1,
                strong: true,
                human_confirmed: id != "c",
                reason: "fixture confirmation".into(),
                evidence_version: "metrics-v1".into(),
            })
            .unwrap();
        }
        for (symbol, status) in [
            (Symbol::Yinli, LimitStatus::Confirmed { consecutive: 1 }),
            (
                Symbol::Seres,
                LimitStatus::Unknown("missing history".into()),
            ),
            (Symbol::Catl, LimitStatus::Confirmed { consecutive: 1 }),
        ] {
            r.add_fact(LimitObservation {
                symbol,
                date: day("2026-10-16"),
                known_at: time("2026-10-16", 16),
                revision: 1,
                source: "fixture".into(),
                status,
            })
            .unwrap();
        }
        r
    }
    fn run(r: &CandidateResearch) -> CandidateSet {
        r.generate(
            day("2026-10-16"),
            time("2026-10-16", 17),
            &[day("2026-10-16"), day("2026-10-19")],
            "v1",
        )
        .unwrap()
    }
    #[test]
    fn confirmed_themes_deduplicate_symbols_and_preserve_unknowns_and_trading_calendar() {
        let result = run(&fixture());
        assert_eq!(result.candidates.len(), 1);
        assert_eq!(result.candidates[0].themes.len(), 2);
        assert_eq!(result.pending.len(), 1);
        assert_eq!(result.applicable_date.as_str(), "2026-10-19");
        let draft = result.draft("draft-100".into()).unwrap();
        assert!(draft.plan.validate_complete().is_err());
        assert_eq!(draft.plan.date, "2026-10-19");
        assert!(draft.plan.research_evidence.is_some());
    }
    #[test]
    fn unconfirmed_system_suggestion_cannot_overwrite_a_human_decision() {
        let mut r = fixture();
        let old = run(&r);
        r.add_assessment(AssessmentObservation {
            id: "new-system-suggestion".into(),
            theme: ThemeId::new("a").unwrap(),
            date: day("2026-10-16"),
            confirmed_at: ResearchTime::new(day("2026-10-16"), 16 * 3600 + 10).unwrap(),
            revision: 2,
            strong: false,
            human_confirmed: false,
            reason: "system only".into(),
            evidence_version: "metrics-v2".into(),
        })
        .unwrap();
        assert_eq!(run(&r), old);
        let mut malformed = serde_json::to_value(&old).unwrap();
        malformed["candidates"][0]["fact_revision"] = serde_json::json!(0);
        assert!(serde_json::from_value::<CandidateSet>(malformed).is_err());
    }
    #[test]
    fn future_corrections_never_change_historical_candidate_snapshot() {
        let mut r = fixture();
        let old = run(&r);
        r.add_fact(LimitObservation {
            symbol: Symbol::Yinli,
            date: day("2026-10-16"),
            known_at: time("2026-10-19", 10),
            revision: 2,
            source: "later correction".into(),
            status: LimitStatus::NotLimit,
        })
        .unwrap();
        r.add_assessment(AssessmentObservation {
            id: "later-judgment".into(),
            theme: ThemeId::new("a").unwrap(),
            date: day("2026-10-16"),
            confirmed_at: time("2026-10-19", 10),
            revision: 2,
            strong: false,
            human_confirmed: true,
            reason: "later evidence".into(),
            evidence_version: "metrics-v2".into(),
        })
        .unwrap();
        assert_eq!(run(&r), old);
    }
    #[test]
    fn rejects_bad_wire_dates_incomplete_calendar_and_preclose_generation() {
        assert!(serde_json::from_str::<TradingDate>("\"2026-02-30\"").is_err());
        assert!(ResearchTime::new(day("2026-10-16"), 86400).is_err());
        let r = fixture();
        assert!(
            r.generate(
                day("2026-10-16"),
                time("2026-10-16", 17),
                &[day("2026-10-16")],
                "v1"
            )
            .is_err()
        );
        assert!(
            r.generate(
                day("2026-10-16"),
                time("2026-10-16", 14),
                &[day("2026-10-16"), day("2026-10-19")],
                "v1"
            )
            .is_err()
        );
    }
}
