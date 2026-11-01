use super::*;
use tempfile::TempDir;

#[test]
fn supplemental_output_asset_commits_next_to_final_audio() {
    let root = TempDir::new().expect("temp root");
    let source = root.path().join("source.pdf");
    std::fs::write(&source, b"%PDF-1.7\nbody").expect("write pdf");
    let final_audio = root.path().join("Book One.m4b");

    let committed = commit_supplemental_output_asset(SupplementalOutputAssetCommitRequest::new(
        &source,
        &final_audio,
    ))
    .expect("commit pdf");

    assert_eq!(
        committed,
        root.path().join("Book One - Supplemental PDF.pdf")
    );
    assert_eq!(
        std::fs::read(committed).expect("read committed"),
        b"%PDF-1.7\nbody"
    );
}

#[test]
fn supplemental_output_asset_renames_without_replacing_existing_pdf() {
    let root = TempDir::new().expect("temp root");
    let source = root.path().join("source.pdf");
    std::fs::write(&source, b"%PDF-1.7\nbody").expect("write pdf");
    let final_audio = root.path().join("Book One.m4b");
    std::fs::write(
        root.path().join("Book One - Supplemental PDF.pdf"),
        b"existing",
    )
    .expect("write existing");

    let committed = commit_supplemental_output_asset(SupplementalOutputAssetCommitRequest::new(
        &source,
        &final_audio,
    ))
    .expect("commit pdf");

    assert_eq!(
        committed,
        root.path().join("Book One - Supplemental PDF (2).pdf")
    );
    assert_eq!(
        std::fs::read(root.path().join("Book One - Supplemental PDF.pdf")).expect("read existing"),
        b"existing"
    );
}

#[test]
fn supplemental_output_asset_rejects_non_pdf_magic() {
    let root = TempDir::new().expect("temp root");
    let source = root.path().join("source.pdf");
    std::fs::write(&source, b"not-pdf").expect("write bad pdf");
    let final_audio = root.path().join("Book One.m4b");

    let error = commit_supplemental_output_asset(SupplementalOutputAssetCommitRequest::new(
        &source,
        &final_audio,
    ))
    .expect_err("bad pdf should fail");

    assert!(error.to_string().contains("magic-byte validation"));
}

#[test]
fn supplemental_output_asset_rejects_oversized_pdf() {
    let root = TempDir::new().expect("temp root");
    let source = root.path().join("source.pdf");
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&source)
        .expect("create source");
    file.set_len(abb_media_core::MAX_SUPPLEMENTAL_PDF_BYTES + 1)
        .expect("make sparse oversized pdf");
    std::fs::write(&source, b"%PDF-").expect("write header");
    file.set_len(abb_media_core::MAX_SUPPLEMENTAL_PDF_BYTES + 1)
        .expect("restore sparse oversized size");
    let final_audio = root.path().join("Book One.m4b");

    let error = commit_supplemental_output_asset(SupplementalOutputAssetCommitRequest::new(
        &source,
        &final_audio,
    ))
    .expect_err("oversized pdf should fail");

    assert!(error.to_string().contains("100 MiB size limit"));
}

#[test]
fn supplemental_output_asset_rejects_stale_expected_hash() {
    let root = TempDir::new().expect("temp root");
    let original_bytes = b"%PDF-1.7\nbody";
    let changed_bytes = b"%PDF-1.7\nchanged";
    let source = root.path().join("source.pdf");
    std::fs::write(&source, original_bytes).expect("write source pdf");
    let request_hash = abb_media_core::sha256_hex(original_bytes);
    std::fs::write(&source, changed_bytes).expect("change source pdf");
    let final_audio = root.path().join("Book One.m4b");

    let error = commit_supplemental_output_asset(
        SupplementalOutputAssetCommitRequest::new(&source, &final_audio)
            .with_expected_identity(changed_bytes.len() as u64, &request_hash),
    )
    .expect_err("stale hash should fail");

    assert!(error.to_string().contains("hash changed"));
    assert!(
        !root.path().join("Book One - Supplemental PDF.pdf").exists(),
        "stale supplemental asset must not be committed"
    );
}
