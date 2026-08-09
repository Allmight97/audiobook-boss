use abb_remote_source_core::AcquisitionStrategy;

const MAX_REMOTE_FILENAME_STEM_BYTES: usize = 180;

/// Stable, non-reversible reference for a provider title id, safe to emit in
/// logs and filenames. Never exposes the raw provider title id.
pub fn title_ref(title_id: &str) -> String {
    abb_media_core::sha256_hex(title_id.as_bytes())
        .chars()
        .take(12)
        .collect()
}

/// File extension for the protected/import-ready download produced by a lane.
pub fn download_extension_for_strategy(strategy: AcquisitionStrategy) -> &'static str {
    match strategy {
        AcquisitionStrategy::DownloadImportReady => "m4b",
        AcquisitionStrategy::DownloadThenDecryptAax => "aax",
        AcquisitionStrategy::DownloadThenDecryptAaxc => "aaxc",
        AcquisitionStrategy::DownloadThenDecryptDash => "mpd",
        AcquisitionStrategy::ProtectedUnsupported | AcquisitionStrategy::ProviderProtocolFailed => {
            "bin"
        }
    }
}

/// Sanitized filename stem for a materialized remote title, falling back to a
/// `title_ref`-derived name when the provider title is missing or unusable.
pub fn remote_materialized_filename_stem(title_name: Option<&str>, title_id: &str) -> String {
    let fallback = format!("Audible {}", title_ref(title_id));
    let Some(title_name) = title_name else {
        return fallback;
    };
    let sanitized = sanitize_remote_filename_stem(title_name);
    if sanitized.is_empty() {
        fallback
    } else {
        truncate_filename_stem(&sanitized, MAX_REMOTE_FILENAME_STEM_BYTES).unwrap_or(fallback)
    }
}

/// User-facing filename for a downloaded Supplemental PDF beside the audiobook.
pub fn supplemental_pdf_display_file_name(title_name: Option<&str>, title_id: &str) -> String {
    format!(
        "{} - Supplemental PDF.pdf",
        remote_materialized_filename_stem(title_name, title_id)
    )
}

fn sanitize_remote_filename_stem(input: &str) -> String {
    let mut value = input.replace(':', " - ");
    value = value.replace(',', " - ");
    value
        .replace(['/', '\\', '*', '?', '"', '<', '>', '|'], " ")
        .chars()
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim_matches(['.', ' ', '-'])
        .to_string()
}

fn truncate_filename_stem(value: &str, max_bytes: usize) -> Option<String> {
    if value.is_empty() || max_bytes == 0 {
        return None;
    }
    if value.len() <= max_bytes {
        return Some(value.to_string());
    }
    let mut end = 0;
    for (index, character) in value.char_indices() {
        let next = index + character.len_utf8();
        if next > max_bytes {
            break;
        }
        end = next;
    }
    let truncated = value[..end]
        .trim_end()
        .trim_matches(['.', ' ', '-'])
        .to_string();
    (!truncated.is_empty()).then_some(truncated)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn download_extension_matches_strategy() {
        assert_eq!(
            download_extension_for_strategy(AcquisitionStrategy::DownloadImportReady),
            "m4b"
        );
        assert_eq!(
            download_extension_for_strategy(AcquisitionStrategy::DownloadThenDecryptAax),
            "aax"
        );
        assert_eq!(
            download_extension_for_strategy(AcquisitionStrategy::DownloadThenDecryptAaxc),
            "aaxc"
        );
        assert_eq!(
            download_extension_for_strategy(AcquisitionStrategy::DownloadThenDecryptDash),
            "mpd"
        );
    }

    #[test]
    fn supplemental_pdf_display_file_name_uses_sanitized_remote_title() {
        let file_name = supplemental_pdf_display_file_name(
            Some("Being You: A New Science of Consciousness"),
            "B000000001",
        );

        assert_eq!(
            file_name,
            "Being You - A New Science of Consciousness - Supplemental PDF.pdf"
        );
    }

    #[test]
    fn supplemental_pdf_display_file_name_sanitizes_path_hostile_title() {
        let file_name = supplemental_pdf_display_file_name(Some("../../bad,title?"), "B000000001");

        assert_eq!(file_name, "bad - title - Supplemental PDF.pdf");
    }

    #[test]
    fn supplemental_pdf_display_file_name_falls_back_to_title_ref_for_empty_title() {
        let title_id = "../../account-title";
        let file_name = supplemental_pdf_display_file_name(Some("../"), title_id);
        let missing_title_file_name = supplemental_pdf_display_file_name(None, title_id);
        let expected = format!("Audible {} - Supplemental PDF.pdf", title_ref(title_id));

        assert_eq!(file_name, expected);
        assert_eq!(missing_title_file_name, expected);
        assert!(!file_name.contains(title_id));
        assert!(!missing_title_file_name.contains(title_id));
    }

    #[test]
    fn remote_filename_stem_uses_sanitized_title() {
        assert_eq!(
            remote_materialized_filename_stem(
                Some("Secure Love: Create a Relationship That Lasts a Lifetime"),
                "B000000001",
            ),
            "Secure Love - Create a Relationship That Lasts a Lifetime"
        );
    }

    #[test]
    fn title_ref_is_stable_and_hides_raw_id() {
        let title_id = "../../account-title";
        assert_eq!(title_ref(title_id).len(), 12);
        assert!(!title_ref(title_id).contains(title_id));
    }
}
