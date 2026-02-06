const AUDNEXUS_ALLOWED_REGIONS: &[&str] =
    &["au", "ca", "de", "es", "fr", "in", "it", "jp", "us", "uk"];

pub(super) fn extract_asin(query: &str) -> Option<String> {
    let mut current = String::new();
    for ch in query.chars() {
        if ch.is_ascii_alphanumeric() {
            current.push(ch.to_ascii_uppercase());
        } else {
            if current.len() == 10 {
                return Some(current);
            }
            current.clear();
        }
    }

    if current.len() == 10 {
        Some(current)
    } else {
        None
    }
}

pub(super) fn extract_region_override(query: &str) -> Option<String> {
    let bytes = query.as_bytes();
    for window in bytes.windows(4) {
        if window[0] == b'[' && window[3] == b']' {
            let region = String::from_utf8_lossy(&window[1..3]).to_ascii_lowercase();
            if AUDNEXUS_ALLOWED_REGIONS.contains(&region.as_str()) {
                return Some(region);
            }
        }
    }
    None
}

pub(super) fn strip_region_overrides(query: &str) -> String {
    let mut output = String::with_capacity(query.len());
    let mut last_end = 0;

    for (start, _) in query.match_indices('[') {
        if start + 4 <= query.len() && query.as_bytes()[start + 3] == b']' {
            let region_candidate = &query[start + 1..start + 3];
            if AUDNEXUS_ALLOWED_REGIONS.contains(&region_candidate.to_ascii_lowercase().as_str()) {
                output.push_str(&query[last_end..start]);
                last_end = start + 4;
            }
        }
    }

    output.push_str(&query[last_end..]);
    output.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub(super) fn region_to_tld(region: &str) -> &str {
    match region {
        "au" => "com.au",
        "ca" => "ca",
        "de" => "de",
        "es" => "es",
        "fr" => "fr",
        "in" => "in",
        "it" => "it",
        "jp" => "co.jp",
        "uk" => "co.uk",
        "us" => "com",
        _ => "com",
    }
}

pub(super) fn parse_year(value: Option<&str>) -> Option<i32> {
    let raw = value?.trim();
    if raw.len() < 4 {
        return None;
    }

    raw.get(0..4)?.parse().ok()
}

pub(super) fn clean_series_part(value: Option<String>) -> Option<String> {
    let raw = value?.trim().to_string();
    if raw.is_empty() {
        return None;
    }
    let mut out = String::new();
    let mut started = false;
    let mut seen_dot = false;

    for ch in raw.chars() {
        if ch.is_ascii_digit() {
            out.push(ch);
            started = true;
            continue;
        }
        if started && ch == '.' && !seen_dot {
            out.push(ch);
            seen_dot = true;
            continue;
        }
        if started {
            break;
        }
    }

    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

pub(super) fn split_primary_series_name(name: &str) -> Option<(String, String)> {
    let (series, rest) = name.split_once(',')?;
    let series = series.trim();
    let rest = rest.trim();
    if series.is_empty() || rest.is_empty() {
        return None;
    }
    let lowered = rest.to_ascii_lowercase();
    let is_prefixed = ["part", "book", "vol", "vol.", "volume"]
        .iter()
        .any(|prefix| lowered.starts_with(prefix));
    if !is_prefixed {
        return None;
    }
    let normalized = rest.replace(": ", " - ");
    Some((series.to_string(), normalized))
}

#[cfg(test)]
mod tests {
    use super::*;

    // EXCEPTION: inline tests for private helper functions.
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
    fn strip_region_overrides_removes_known_regions() {
        let query = "[uk] The Martian";
        assert_eq!(strip_region_overrides(query), "The Martian");
        let query = "The Martian [us]";
        assert_eq!(strip_region_overrides(query), "The Martian");
    }
}
