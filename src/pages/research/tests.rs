use super::{DATES, sample_rows};

#[test]
fn source_order_is_independent_for_each_day_and_zeroes_survive() {
    let first = sample_rows(DATES[0], 0);
    let second = sample_rows(DATES[1], 1);
    assert_eq!(first[0].sector, "AI 应用");
    assert_eq!(second[0].sector, "汽车产业链");
    assert!(first.iter().any(|r| r.symbol.code() == "001318"));
}
