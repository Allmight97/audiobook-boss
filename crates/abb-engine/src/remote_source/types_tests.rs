use super::RemoteAcquisitionFailureKind::{self, *};

#[test]
fn only_a_leftover_or_a_missing_optional_pdf_leaves_the_title_importable() {
    let importable_despite: Vec<RemoteAcquisitionFailureKind> = [
        AuthRequired,
        ProviderPrivateProtocolFailed,
        LicenseDenied,
        ProtectedUnsupported,
        DownloadFailed,
        MaterializationFailed,
        ProtectedSourcePurgeFailed,
        ValidationFailed,
        SupplementalPdfFailed,
        SupplementalPdfUnavailable,
        ReleaseSearchFailed,
        ReleaseGrabFailed,
        Cancelled,
    ]
    .into_iter()
    .filter(|kind| !kind.blocks_title())
    .collect();

    assert_eq!(
        importable_despite,
        [ProtectedSourcePurgeFailed, SupplementalPdfUnavailable]
    );
}
