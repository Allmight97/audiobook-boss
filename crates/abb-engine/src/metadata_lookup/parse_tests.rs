use super::*;

#[test]
fn extract_asin_finds_first_asin() {
    let query = "Project Hail Mary B01234ABCD [uk]";
    assert_eq!(extract_asin(query), Some("B01234ABCD".to_string()));
}

#[test]
fn extract_region_override_accepts_known_regions() {
    let query = "B01234ABCD [uk]";
    assert_eq!(extract_region_override(query), Some("uk".to_string()));
    let query = "[us] The Martian";
    assert_eq!(extract_region_override(query), Some("us".to_string()));
}

#[test]
fn split_primary_series_name_parses_part() {
    let input = "Frontiers Saga, Part 3: Fringe Worlds";
    let (series, subseries) =
        split_primary_series_name(input).expect("expected to parse primary series name");
    assert_eq!(series, "Frontiers Saga");
    assert_eq!(subseries, "Part 3 - Fringe Worlds");
}

#[test]
fn clean_series_part_extracts_number() {
    assert_eq!(
        clean_series_part(Some("Book 12".to_string())),
        Some("12".to_string())
    );
    assert_eq!(
        clean_series_part(Some("Part 3.5".to_string())),
        Some("3.5".to_string())
    );
    assert_eq!(clean_series_part(Some("Nope".to_string())), None);
    assert_eq!(clean_series_part(Some("".to_string())), None);
}

#[test]
fn parse_publication_date_accepts_year_month_and_date_prefixes() {
    assert_eq!(
        parse_publication_date(Some("2024")),
        Some("2024".to_string())
    );
    assert_eq!(
        parse_publication_date(Some(" 2024-07 ")),
        Some("2024-07".to_string())
    );
    assert_eq!(
        parse_publication_date(Some("2024-07-15")),
        Some("2024-07".to_string())
    );
    assert_eq!(parse_publication_date(Some("99")), None);
    assert_eq!(parse_publication_date(Some("2024-13")), None);
    assert_eq!(parse_publication_date(Some("abcd")), None);
}

#[test]
fn strip_region_overrides_removes_known_regions() {
    let query = "[uk] The Martian";
    assert_eq!(strip_region_overrides(query), "The Martian");
    let query = "The Martian [us]";
    assert_eq!(strip_region_overrides(query), "The Martian");
}
