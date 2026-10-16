//! Document invariants exercised through the public core contract.
use tydaro_core::desktop::{DEMO_DATE, seed, validate};

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
