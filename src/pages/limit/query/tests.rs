use super::super::{DATES, PageId, Symbol, sample_rows, seed_rows};
use super::{Field, Filter, matches, restore_rows, source};

#[test]
fn filter_intersection_is_not_union_and_removal_restores_rows() {
    let rows = seed_rows();
    let filters = vec![
        Filter {
            field: Field::Sector,
            value: "AI".into(),
        },
        Filter {
            field: Field::Date,
            value: "2026-10-14".into(),
        },
    ];
    let result: Vec<_> = rows.iter().filter(|r| matches(r, &filters)).collect();
    assert_eq!(result.len(), 3);
    assert!(
        result
            .iter()
            .all(|r| r.date == DATES[0] && r.sector == "AI 应用")
    );
    assert!(rows.iter().filter(|r| matches(r, &filters[..1])).count() > result.len());
}

#[test]
fn handoff_retains_exact_date_reason_and_deduplicates_symbols() {
    let rows = sample_rows("2026-10-08", 0);
    let mut selected = vec![rows[0].clone(), rows[0].clone()];
    selected[0].reason = "历史依据".into();
    let context = source(&selected, PageId::Limit);
    assert_eq!(context.date, "2026-10-08");
    assert_eq!(context.reason, "历史依据");
    assert_eq!(context.symbols, vec![Symbol::Yinli]);
}

#[test]
fn undo_after_reimport_preserves_current_edits_without_duplicate_ids() {
    let deleted = sample_rows("2026-10-15", 0);
    let mut current = deleted.clone();
    current[0].reason = "重新导入后的人工修订".into();
    let count = current.len();
    restore_rows(&mut current, deleted);
    assert_eq!(current.len(), count);
    assert_eq!(current[0].reason, "重新导入后的人工修订");
}
