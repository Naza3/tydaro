//! Unit regressions for this module.

use super::available_name;

#[test]
fn long_duplicate_names_stay_valid_and_unique() {
    let base = format!("{} 副本", "盘".repeat(20));
    let mut boards = vec![(1, base.clone())];
    for id in 2..=16 {
        let name = available_name(&base, &boards);
        assert_eq!(
            crate::workspace::boards::validate_board_name(&name).unwrap(),
            name
        );
        assert!(!boards.iter().any(|(_, existing)| existing == &name));
        boards.push((id, name));
    }
    let name = available_name(&"盘".repeat(30), &[]);
    assert_eq!(name.chars().count(), 24);
    assert!(crate::workspace::boards::validate_board_name(&name).is_ok());
}
