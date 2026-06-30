use audiobook_boss_lib::processing::progress::converting_percentage_from_seconds;

#[test]
fn test_converting_percentage_from_seconds() {
    // Test basic calculation
    assert!(converting_percentage_from_seconds(0.0, 100.0) >= 10.0);
    assert!(converting_percentage_from_seconds(50.0, 100.0) > 10.0);
    assert!(converting_percentage_from_seconds(100.0, 100.0) <= 80.0);

    // Test edge cases
    assert_eq!(
        converting_percentage_from_seconds(0.0, 0.0),
        10.0 // PROGRESS_CONVERTING_START
    );
    assert!(converting_percentage_from_seconds(100.0, 0.0) >= 10.0);
}
