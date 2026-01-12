use audiobook_boss_lib::commands::metadata::compute_tsoa;

#[test]
fn computes_tsoa_with_numeric_part() {
    let result = compute_tsoa("Series", Some("3"), "Title");
    assert_eq!(result.as_deref(), Some("Series 03 - Title"));
}

#[test]
fn skips_tsoa_with_fractional_part() {
    let result = compute_tsoa("Series", Some("1/5"), "Title");
    assert!(result.is_none());
}

#[test]
fn skips_tsoa_when_part_missing_or_invalid() {
    assert!(compute_tsoa("Series", None, "Title").is_none());
    assert!(compute_tsoa("Series", Some(""), "Title").is_none());
    assert!(compute_tsoa("Series", Some("abc"), "Title").is_none());
    assert!(compute_tsoa("Series", Some("0"), "Title").is_none());
}
