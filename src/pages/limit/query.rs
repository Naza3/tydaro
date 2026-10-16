use super::*;

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Field {
    All,
    Sector,
    Code,
    Name,
    Board,
    Reason,
    Date,
    Source,
}
impl Field {
    pub(super) const ALL: [Self; 8] = [
        Self::All,
        Self::Sector,
        Self::Code,
        Self::Name,
        Self::Board,
        Self::Reason,
        Self::Date,
        Self::Source,
    ];
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::All => "全部字段",
            Self::Sector => "板块",
            Self::Code => "股票代码",
            Self::Name => "股票名称",
            Self::Board => "连板字段",
            Self::Reason => "关键词",
            Self::Date => "交易日",
            Self::Source => "来源文件",
        }
    }
    fn value(self, row: &ResearchRow) -> String {
        match self {
            Self::All => format!(
                "{} {} {} {} {} {} {}",
                row.date,
                row.sector,
                row.symbol.code(),
                row.name,
                row.board,
                row.reason,
                row.source
            ),
            Self::Sector => row.sector.clone(),
            Self::Code => row.symbol.code().to_owned(),
            Self::Name => row.name.clone(),
            Self::Board => row.board.clone(),
            Self::Reason => row.reason.clone(),
            Self::Date => row.date.clone(),
            Self::Source => row.source.clone(),
        }
    }
}
#[derive(Clone)]
pub(super) struct Filter {
    pub(super) field: Field,
    pub(super) value: String,
}

pub(super) fn restore_rows(rows: &mut Vec<ResearchRow>, restored: Vec<ResearchRow>) {
    // Reimporting a deleted day before undo must not duplicate IDs or replace
    // records subsequently edited by the user.
    for row in restored {
        if !rows.iter().any(|existing| existing.id == row.id) {
            rows.push(row);
        }
    }
    rows.sort_by(|a, b| b.date.cmp(&a.date));
}
pub(super) fn matches(row: &ResearchRow, filters: &[Filter]) -> bool {
    filters.iter().all(|filter| {
        filter
            .field
            .value(row)
            .to_lowercase()
            .contains(&filter.value.to_lowercase())
    })
}
pub(super) fn source(rows: &[ResearchRow], origin: PageId) -> SourceContext {
    let mut symbols = Vec::new();
    for row in rows {
        if !symbols.contains(&row.symbol) {
            symbols.push(row.symbol);
        }
    }
    SourceContext {
        date: rows.first().map_or(DATES[0], |r| &r.date).into(),
        sector: rows.first().map_or("研究观察", |r| &r.sector).into(),
        symbols,
        reason: rows.first().map_or("", |r| &r.reason).into(),
        origin,
    }
}

#[cfg(test)]
mod tests;
