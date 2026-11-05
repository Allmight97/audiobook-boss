use super::*;
use crate::remote_source::vault::KeyringSecretVault;
use abb_audible_core::supplemental_pdf_display_file_name;

#[tokio::test]
#[ignore = "uses local keychain Audible auth and a real owned title"]
async fn audible_pdf_live_probe() {
    let title_id = std::env::var("ABB_AUDIBLE_PDF_PROBE_TITLE_ID")
        .expect("ABB_AUDIBLE_PDF_PROBE_TITLE_ID is required");
    let vault = KeyringSecretVault::for_app_identifier("com.audiobook-boss");
    let auth = auth_from_vault(&vault).expect("Audible account must be connected");
    let root = tempfile::TempDir::new().expect("temp root");
    let file_name = supplemental_pdf_display_file_name(None, &title_id);

    let (asset, committed) = super::supplemental_pdf::download_supplemental_pdf(
        super::supplemental_pdf::SupplementalPdfRequest {
            auth: &auth,
            title_id: &title_id,
            job_id: "audible-pdf-live-probe",
            input_id: "probe-input",
            file_name: &file_name,
            api_pdf_hint_present: true,
            job_dir: root.path(),
        },
        &|| false,
    )
    .await
    .expect("download supplemental PDF");
    committed.permanent();
    assert!(std::fs::read(&asset.path)
        .expect("read staged PDF")
        .starts_with(b"%PDF-"));

    let final_audio = root.path().join("Probe.m4b");
    std::fs::write(&final_audio, b"audio").expect("write dummy final audio");
    let committed = crate::output_artifact::commit_supplemental_output_asset(
        crate::output_artifact::SupplementalOutputAssetCommitRequest::new(
            &asset.path,
            &final_audio,
        ),
    )
    .expect("commit supplemental PDF beside dummy final audio");
    assert!(std::fs::read(committed)
        .expect("read committed PDF")
        .starts_with(b"%PDF-"));
}
