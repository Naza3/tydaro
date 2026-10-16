use super::{actual_date, valid_date};

#[test]
fn cutoff_never_uses_future_data_and_calendar_input_is_validated() {
    assert_eq!(actual_date("2026-10-11"), Some("2026-10-09"));
    assert_eq!(actual_date("2026-09-01"), None);
    assert_eq!(actual_date("2026-02-30"), None);
    assert!(valid_date("2024-02-29"));
    assert!(!valid_date("2026-02-29"));
}
