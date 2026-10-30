use super::*;
#[cfg(unix)]
use std::ffi::OsString;
#[cfg(unix)]
use std::os::unix::ffi::OsStringExt;
#[cfg(unix)]
use std::path::PathBuf;

#[test]
fn classifies_media_containers_by_extension() {
    assert_eq!(
        classify_media_container_path(Path::new("book.m4b")),
        MediaContainerKind::M4b
    );
    assert_eq!(
        classify_media_container_path(Path::new("book.aax")),
        MediaContainerKind::Aax
    );
    assert_eq!(
        classify_media_container_path(Path::new("book.aaxc")),
        MediaContainerKind::Aaxc
    );
    assert_eq!(
        classify_media_container_path(Path::new("book.mpd")),
        MediaContainerKind::Dash
    );
}

#[test]
fn maps_container_and_drm_to_protection_kind() {
    assert_eq!(
        protection_for_container(MediaContainerKind::M4b, None),
        MediaProtectionKind::None
    );
    assert_eq!(
        protection_for_container(MediaContainerKind::Aax, Some("Mpeg")),
        MediaProtectionKind::AudibleAax
    );
    assert_eq!(
        protection_for_container(MediaContainerKind::Aaxc, Some("Mpeg")),
        MediaProtectionKind::AudibleAaxc
    );
    assert_eq!(
        protection_for_container(MediaContainerKind::M4b, Some("Widevine")),
        MediaProtectionKind::Widevine
    );
}

#[test]
fn only_m4b_family_is_import_ready_audio() {
    assert!(container_is_import_ready_audio(MediaContainerKind::M4b));
    assert!(container_is_import_ready_audio(MediaContainerKind::M4a));
    assert!(!container_is_import_ready_audio(MediaContainerKind::Aax));
    assert!(!container_is_import_ready_audio(MediaContainerKind::Aaxc));
    assert!(!container_is_import_ready_audio(MediaContainerKind::Dash));
}

#[cfg(unix)]
#[test]
fn non_utf8_extensions_classify_as_unknown() {
    let path = PathBuf::from(OsString::from_vec(b"book.\xFFaax".to_vec()));

    assert_eq!(
        classify_media_container_path(&path),
        MediaContainerKind::Unknown
    );
}

#[test]
fn pdf_magic_detects_only_pdf_header() {
    assert!(has_pdf_magic(b"%PDF-1.7\nbody"));
    assert!(!has_pdf_magic(b"not-a-pdf"));
    assert!(!has_pdf_magic(b""));
    assert!(!has_pdf_magic(b"%PDF"));
}

#[test]
fn supplemental_pdf_limit_is_100_mib() {
    assert_eq!(MAX_SUPPLEMENTAL_PDF_BYTES, 104_857_600);
}

#[test]
fn sha256_hex_matches_known_vector() {
    assert_eq!(
        sha256_hex(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert_eq!(
        sha256_hex(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

#[test]
fn supplemental_pdf_streaming_identity_matches_chunked_pdf() {
    let mut identity = SupplementalPdfIdentityBuilder::new();
    identity.update(b"%P");
    identity.update(b"DF-1.7\nbody");

    let identity = identity.finalize();

    assert_eq!(identity.size_bytes, 13);
    assert!(identity.has_pdf_magic);
    assert_eq!(identity.sha256, sha256_hex(b"%PDF-1.7\nbody"));
}

#[test]
fn supplemental_pdf_short_stream_is_not_pdf_magic() {
    let mut identity = SupplementalPdfIdentityBuilder::new();
    identity.update(b"%PDF");

    assert!(!identity.finalize().has_pdf_magic);
}
