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
