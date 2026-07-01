use std::path::Path;

mod paths;
mod progress;
mod supplemental;
mod validation;

use super::audio_download::download_audio;
use super::diagnostics::AudibleAcquisitionError;
use super::license::{
    license_decrypt_context_from_auth, lookup_title_details, provider_protocol_lane_message,
    request_license_lane, strategy_label, AudibleTitleDetails, LicenseLane,
};
use super::materialization::materialize_protected_download;
use super::{auth_from_vault, client_from_auth};
use crate::errors::Result;
use crate::remote_source::materializer::AaxcleanMaterializer;
use crate::remote_source::scoped_output::ProvisionalCommittedFile;
use crate::remote_source::staging;
use crate::remote_source::vault::SecretVault;
use crate::remote_source::{
    AcquisitionJob, AcquisitionPlan, MaterializedSourceFile, ProviderId,
    RemoteAcquisitionFailureKind, RemoteAcquisitionStatus, RemoteSourceDiagnostic,
    SupplementalAsset,
};
use abb_audible_core::{title_ref, AudibleLicenseDecryptContext};
use abb_remote_source_core::{
    acquisition_progress, AcquisitionProgress, AcquisitionStage, AcquisitionStrategy,
};
use audible_api::api::Client as AudibleClient;
use audible_api::auth::Auth;

pub(super) struct AcquiredTitle {
    pub(super) file: Option<MaterializedSourceFile>,
    pub(super) assets: Vec<SupplementalAsset>,
    pub(super) diagnostics: Vec<RemoteSourceDiagnostic>,
}

type AudibleAcquisitionResult<T> = std::result::Result<T, AudibleAcquisitionError>;

pub(in crate::remote_source::providers::audible) use paths::{
    generated_staging_path, staged_materialized_path, staged_protected_source_path,
};
pub(in crate::remote_source::providers::audible) use progress::{
    title_progress, with_title_progress, TitleProgressContext,
};

struct TitleAcquisitionRequest<'a> {
    title_id: &'a str,
    include_pdf: bool,
    job_id: &'a str,
    job_dir: &'a Path,
    license_decrypt_context: Option<&'a AudibleLicenseDecryptContext>,
    progress_context: TitleProgressContext<'a>,
}

struct FinalizeAcquisitionRequest<'a> {
    auth: &'a Auth,
    materialized_path: std::path::PathBuf,
    include_pdf: bool,
    title_details: &'a AudibleTitleDetails,
    lane: &'a LicenseLane,
    ctx: TitleAcquisitionCtx<'a>,
}

#[derive(Clone, Copy)]
pub(in crate::remote_source::providers::audible) struct TitleAcquisitionCtx<'a> {
    pub(super) job_id: &'a str,
    pub(super) title_id: &'a str,
    pub(super) item_id: &'a str,
    pub(super) item_dir: &'a Path,
    pub(super) progress_context: TitleProgressContext<'a>,
}

pub(super) async fn acquire(
    vault: &dyn SecretVault,
    materializer: &AaxcleanMaterializer,
    plan: &AcquisitionPlan,
    job_id: &str,
    job_dir: &Path,
    mut progress: impl FnMut(AcquisitionProgress),
    is_cancelled: impl Fn() -> bool,
) -> Result<AcquisitionJob> {
    ensure_not_cancelled(&is_cancelled)?;
    let auth = auth_from_vault(vault)?;
    let license_decrypt_context = license_decrypt_context_from_auth(&auth);
    let client = client_from_auth(auth.clone())?;
    let mut job = AcquisitionJob {
        job_id: job_id.to_string(),
        provider_id: ProviderId::Audible,
        status: RemoteAcquisitionStatus::Acquiring,
        progress: acquisition_progress(AcquisitionStage::License, Some(0.0), None, None),
        materialized_files: Vec::new(),
        supplemental_assets: Vec::new(),
        diagnostics: Vec::new(),
    };

    let total_items = u32::try_from(plan.selections.len()).unwrap_or(u32::MAX);
    for (selection_index, selection) in plan.selections.iter().enumerate() {
        ensure_not_cancelled(&is_cancelled)?;
        let request = TitleAcquisitionRequest {
            title_id: &selection.title_id,
            include_pdf: selection.include_supplemental_pdf,
            job_id,
            job_dir,
            license_decrypt_context: license_decrypt_context.as_ref(),
            progress_context: TitleProgressContext {
                title_id: &selection.title_id,
                item_index: u32::try_from(selection_index + 1).unwrap_or(u32::MAX),
                total_items,
            },
        };
        match acquire_one(
            &client,
            &auth,
            materializer,
            request,
            &mut progress,
            &is_cancelled,
        )
        .await
        {
            Ok(AcquiredTitle {
                file: Some(file),
                assets,
                diagnostics,
            }) => {
                job.materialized_files.push(file);
                job.supplemental_assets.extend(assets);
                job.diagnostics.extend(diagnostics);
            }
            Ok(AcquiredTitle {
                file: None,
                assets,
                diagnostics,
            }) => {
                job.supplemental_assets.extend(assets);
                job.diagnostics.extend(diagnostics);
                job.diagnostics.push(RemoteSourceDiagnostic {
                    kind: RemoteAcquisitionFailureKind::ProtectedUnsupported,
                    title_id: Some(selection.title_id.clone()),
                    message: "Audible returned provider data, but no import-compatible audio was materialized.".to_string(),
                });
            }
            Err(error) => {
                if error.is_cancellation() {
                    return Err(error.into_app_error());
                }
                job.diagnostics
                    .push(error.into_diagnostic(Some(selection.title_id.clone())));
            }
        }
    }

    job.status = if job.materialized_files.is_empty() {
        RemoteAcquisitionStatus::Failed
    } else {
        RemoteAcquisitionStatus::Validated
    };
    job.progress = if job.status == RemoteAcquisitionStatus::Failed {
        acquisition_progress(AcquisitionStage::Failed, Some(1.0), None, None)
    } else {
        acquisition_progress(AcquisitionStage::ImportHandoff, Some(1.0), None, None)
    };
    Ok(job)
}

pub(in crate::remote_source::providers::audible) use crate::remote_source::cancellation::{
    ensure_not_cancelled, remote_acquisition_cancelled,
};

async fn acquire_one(
    client: &AudibleClient,
    auth: &Auth,
    materializer: &AaxcleanMaterializer,
    request: TitleAcquisitionRequest<'_>,
    progress: &mut impl FnMut(AcquisitionProgress),
    is_cancelled: &impl Fn() -> bool,
) -> AudibleAcquisitionResult<AcquiredTitle> {
    ensure_not_cancelled(is_cancelled).map_err(AudibleAcquisitionError::cancellation)?;
    let TitleAcquisitionRequest {
        title_id,
        include_pdf,
        job_id,
        job_dir,
        license_decrypt_context,
        progress_context,
    } = request;
    let item_id = uuid::Uuid::new_v4().to_string();
    let item_dir = staging::create_item_dir(job_dir, &item_id)
        .map_err(AudibleAcquisitionError::materialization)?;
    let ctx = TitleAcquisitionCtx {
        job_id,
        title_id,
        item_id: &item_id,
        item_dir: &item_dir,
        progress_context,
    };
    log::info!(
        "remote_source audible stage=title_start job_id={} title_ref={} include_pdf={}",
        job_id,
        title_ref(title_id),
        include_pdf
    );
    progress(title_progress(
        progress_context,
        AcquisitionStage::License,
        Some(0.05),
        None,
        None,
    ));
    let title_details = lookup_title_details(client, ctx, include_pdf, is_cancelled)
        .await
        .map_err(AudibleAcquisitionError::provider_protocol)?;
    let title_name = title_details.title.as_deref();
    let lane = request_license_lane(client, ctx, license_decrypt_context, progress, is_cancelled)
        .await
        .map_err(AudibleAcquisitionError::provider_protocol)?;
    if let Some(unsupported) = unsupported_result_for_unmaterializable_lane(title_id, job_id, &lane)
    {
        return Ok(unsupported);
    }

    let downloaded_path = if lane.strategy == AcquisitionStrategy::DownloadImportReady {
        staged_materialized_path(&item_dir, title_name, title_id)
    } else {
        staged_protected_source_path(&item_dir, lane.strategy)
    };
    download_audio(
        &lane.content_url,
        &downloaded_path,
        ctx,
        progress,
        is_cancelled,
    )
    .await
    .map_err(AudibleAcquisitionError::download)?;
    ensure_not_cancelled(is_cancelled).map_err(AudibleAcquisitionError::cancellation)?;
    progress(title_progress(
        progress_context,
        AcquisitionStage::Validation,
        Some(0.1),
        None,
        None,
    ));
    let (materialized_path, purge_diagnostic) =
        if lane.strategy == AcquisitionStrategy::DownloadImportReady {
            (downloaded_path, None)
        } else {
            let materialized = materialize_protected_download(
                materializer,
                &downloaded_path,
                title_name,
                &lane,
                ctx,
                progress,
                is_cancelled,
            )
            .await
            .map_err(AudibleAcquisitionError::materialization)?;
            (materialized.path, materialized.purge_diagnostic)
        };
    let mut title = finalize_acquired_title(
        FinalizeAcquisitionRequest {
            auth,
            materialized_path,
            include_pdf,
            title_details: &title_details,
            lane: &lane,
            ctx,
        },
        progress,
        is_cancelled,
    )
    .await?;
    // Purge failure is housekeeping, not acquisition failure: the title stays
    // import-ready and the diagnostic rides along non-blocking (#393).
    title.diagnostics.extend(purge_diagnostic);
    Ok(title)
}

async fn finalize_acquired_title(
    request: FinalizeAcquisitionRequest<'_>,
    progress: &mut impl FnMut(AcquisitionProgress),
    is_cancelled: &impl Fn() -> bool,
) -> AudibleAcquisitionResult<AcquiredTitle> {
    let FinalizeAcquisitionRequest {
        auth,
        materialized_path,
        include_pdf,
        title_details,
        lane,
        ctx,
    } = request;
    let title_name = title_details.title.as_deref();
    let committed_audio = ProvisionalCommittedFile::new(materialized_path);
    let file = validation::validate_materialized_audio(committed_audio.path(), ctx, progress)
        .await
        .map_err(AudibleAcquisitionError::validation)?;
    let supplemental_pdf_hint_present =
        supplemental::hint_present_for_acquisition(include_pdf, title_details, lane);
    let (assets, diagnostics) = supplemental::download_if_requested(
        supplemental::SupplementalPdfAcquisitionRequest {
            auth,
            file: &file,
            title_name,
            include_pdf,
            api_pdf_hint_present: supplemental_pdf_hint_present,
            ctx,
        },
        is_cancelled,
    )
    .await?;
    committed_audio.permanent();
    Ok(AcquiredTitle {
        file: Some(file),
        assets,
        diagnostics,
    })
}

pub(super) fn unsupported_result_for_unmaterializable_lane(
    title_id: &str,
    job_id: &str,
    lane: &LicenseLane,
) -> Option<AcquiredTitle> {
    let strategy = lane.strategy;
    match strategy {
        AcquisitionStrategy::DownloadImportReady
        | AcquisitionStrategy::DownloadThenDecryptAax
        | AcquisitionStrategy::DownloadThenDecryptAaxc => None,
        AcquisitionStrategy::DownloadThenDecryptDash => {
            let lane = strategy_label(strategy);
            log::info!(
                "remote_source audible stage=materializer_selection job_id={} title_ref={} lane={} action=unsupported",
                job_id,
                title_ref(title_id),
                lane
            );
            Some(AcquiredTitle {
                file: None,
                assets: Vec::new(),
                diagnostics: vec![RemoteSourceDiagnostic {
                    kind: RemoteAcquisitionFailureKind::ProtectedUnsupported,
                    title_id: Some(title_id.to_string()),
                    message: format!(
                        "Audible returned {lane}; Dash/Widevine materialization is not supported in this build."
                    ),
                }],
            })
        }
        AcquisitionStrategy::ProviderProtocolFailed | AcquisitionStrategy::ProtectedUnsupported => {
            log::warn!(
                "remote_source audible stage=license_classification job_id={} title_ref={} lane={} action=failed",
                job_id,
                title_ref(title_id),
                strategy_label(strategy)
            );
            Some(AcquiredTitle {
                file: None,
                assets: Vec::new(),
                diagnostics: vec![RemoteSourceDiagnostic {
                    kind: if strategy == AcquisitionStrategy::ProviderProtocolFailed {
                        RemoteAcquisitionFailureKind::ProviderPrivateProtocolFailed
                    } else {
                        RemoteAcquisitionFailureKind::ProtectedUnsupported
                    },
                    title_id: Some(title_id.to_string()),
                    message: provider_protocol_lane_message(strategy),
                }],
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errors::AppError;
    use crate::remote_source::providers::audible::audio_download::download_status_failure;
    use crate::remote_source::scoped_output::{partial_sibling, rollback_committed_file};
    use secrecy::ExposeSecret;
    use serde_json::json;
    use std::path::Path;

    #[test]
    fn generated_staging_path_does_not_use_provider_title_id() {
        let root = Path::new("/tmp/abb-remote-job");
        let unsafe_title_id = "../../account-title";

        let path = generated_staging_path(root, "m4b");

        assert!(path.starts_with(root));
        assert!(!path.to_string_lossy().contains(unsafe_title_id));
        assert_eq!(
            path.extension().and_then(|value| value.to_str()),
            Some("m4b")
        );
    }

    #[test]
    fn item_staged_paths_do_not_use_provider_title_id() {
        let root = tempfile::TempDir::new().expect("temp root");
        let item_dir = staging::create_item_dir(root.path(), "item-1").expect("item dir");
        let unsafe_title_id = "../../account-title";

        let protected_path =
            staged_protected_source_path(&item_dir, AcquisitionStrategy::DownloadThenDecryptAaxc);
        let materialized_path =
            staged_materialized_path(&item_dir, Some("../../account:title,?"), unsafe_title_id);

        assert!(protected_path.starts_with(root.path()));
        assert!(materialized_path.starts_with(root.path()));
        assert!(!protected_path.to_string_lossy().contains(unsafe_title_id));
        assert!(!materialized_path
            .to_string_lossy()
            .contains(unsafe_title_id));
        assert_eq!(
            protected_path.file_name().and_then(|value| value.to_str()),
            Some("source.aaxc")
        );
        assert_eq!(
            materialized_path
                .file_name()
                .and_then(|value| value.to_str()),
            Some("account - title.m4b")
        );
    }

    #[test]
    fn staged_materialized_path_uses_sanitized_remote_title() {
        let root = tempfile::TempDir::new().expect("temp root");
        let item_dir = staging::create_item_dir(root.path(), "item-1").expect("item dir");

        let materialized_path = staged_materialized_path(
            &item_dir,
            Some("Secure Love: Create a Relationship That Lasts a Lifetime"),
            "B000000001",
        );

        assert!(materialized_path.starts_with(root.path()));
        assert_eq!(
            materialized_path
                .file_name()
                .and_then(|value| value.to_str()),
            Some("Secure Love - Create a Relationship That Lasts a Lifetime.m4b")
        );
    }

    #[test]
    fn staged_materialized_path_falls_back_to_title_ref_for_empty_title() {
        let root = tempfile::TempDir::new().expect("temp root");
        let item_dir = staging::create_item_dir(root.path(), "item-1").expect("item dir");
        let title_id = "../../account-title";

        let materialized_path = staged_materialized_path(&item_dir, Some("../"), title_id);
        let expected = format!("Audible {}.m4b", title_ref(title_id));

        assert!(materialized_path.starts_with(root.path()));
        assert_eq!(
            materialized_path
                .file_name()
                .and_then(|value| value.to_str()),
            Some(expected.as_str())
        );
        assert!(!materialized_path.to_string_lossy().contains(title_id));
    }

    #[test]
    fn sha256_file_streams_expected_digest() {
        let root = tempfile::TempDir::new().expect("temp root");
        let path = root.path().join("book.m4b");
        let bytes = b"materialized-audio";
        std::fs::write(&path, bytes).expect("write source");

        assert_eq!(
            paths::sha256_file(&path).expect("hash file"),
            abb_media_core::sha256_hex(bytes)
        );
    }

    #[test]
    fn cancellation_guard_returns_cancellation_error() {
        let error = ensure_not_cancelled(&|| true).expect_err("cancelled");

        assert!(matches!(error, AppError::Cancellation(_)));
        assert!(error
            .to_string()
            .contains("Remote source acquisition was cancelled"));
    }

    #[test]
    fn cancellation_guard_allows_active_acquisition() {
        ensure_not_cancelled(&|| false).expect("active acquisition");
    }

    #[test]
    fn encrypted_download_is_not_reported_as_materialized_without_decryption() {
        let root = tempfile::TempDir::new().expect("temp root");
        let encrypted = root.path().join("book.aax");
        std::fs::write(&encrypted, b"encrypted").expect("write encrypted fixture");

        let result = validation::materialized_file_from_path("B000000001", &encrypted);

        let error = result.expect_err("encrypted download must not be import-ready");
        assert!(error.to_string().contains("requires Audible decryption"));
    }

    #[test]
    fn dash_lane_is_reported_unsupported_not_deferred() {
        let lane = LicenseLane {
            content_url: "https://cdn.example.test/manifest.mpd".to_string(),
            strategy: AcquisitionStrategy::DownloadThenDecryptDash,
            decryption_material: None,
            supplemental_pdf_url: None,
        };

        let acquired = unsupported_result_for_unmaterializable_lane("B000000001", "job-1", &lane)
            .expect("dash unsupported");

        assert!(acquired.file.is_none());
        assert_eq!(
            acquired.diagnostics[0].kind,
            RemoteAcquisitionFailureKind::ProtectedUnsupported
        );
        assert!(acquired.diagnostics[0]
            .message
            .contains("Dash/Widevine materialization is not supported"));
    }

    #[test]
    fn rollback_committed_file_removes_committed_staged_output() {
        let root = tempfile::TempDir::new().expect("temp root");
        let final_path = root.path().join("book.aax");
        std::fs::write(&final_path, b"encrypted").expect("write final");

        rollback_committed_file(&final_path).expect("rollback committed file");

        assert!(!final_path.exists());
        assert!(!partial_sibling(&final_path).exists());
    }

    #[test]
    fn typed_acquisition_errors_map_to_diagnostics_without_message_matching() {
        let error = AudibleAcquisitionError::download(download_status_failure(403));

        assert_eq!(error.kind(), RemoteAcquisitionFailureKind::DownloadFailed);
        let diagnostic = error.into_diagnostic(Some("B000000001".to_string()));

        assert_eq!(
            diagnostic.kind,
            RemoteAcquisitionFailureKind::DownloadFailed
        );
        assert_eq!(diagnostic.title_id.as_deref(), Some("B000000001"));
        assert!(!diagnostic.message.contains("token"));
        assert!(!diagnostic.message.contains("license"));
    }

    #[test]
    fn helper_lane_follows_voucher_key_shape_not_content_url_extension() {
        use crate::remote_source::materializer::{AaxcleanLane, AaxcleanSecret};
        use crate::remote_source::providers::audible::materialization::helper_material_from_audible_material;
        use abb_audible_core::audible_decryption_material_from_license;
        use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};

        let response = json!({
            "content_license": {
                "content_url": "https://cdn.example.test/book.aax",
                "drm_type": "Mpeg",
                "voucher": {
                    "key": URL_SAFE_NO_PAD.encode([
                        0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
                        0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f, 0x10
                    ]),
                    "iv": URL_SAFE_NO_PAD.encode([
                        0x10, 0x0f, 0x0e, 0x0d, 0x0c, 0x0b, 0x0a, 0x09,
                        0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01
                    ])
                }
            }
        });
        let material = audible_decryption_material_from_license(
            &response,
            AcquisitionStrategy::DownloadThenDecryptAax,
            "B000000001",
            None,
        )
        .expect("aaxc-shaped material");
        let lane = LicenseLane {
            content_url: "https://cdn.example.test/book.aax".to_string(),
            strategy: AcquisitionStrategy::DownloadThenDecryptAax,
            decryption_material: Some(material),
            supplemental_pdf_url: None,
        };

        let (helper_lane, secret) =
            helper_material_from_audible_material(&lane).expect("helper material");

        assert_eq!(helper_lane, AaxcleanLane::Aaxc);
        match secret {
            AaxcleanSecret::Aaxc { key_hex, iv_hex } => {
                assert_eq!(key_hex.expose_secret(), "0102030405060708090a0b0c0d0e0f10");
                assert_eq!(iv_hex.expose_secret(), "100f0e0d0c0b0a090807060504030201");
            }
            AaxcleanSecret::Aax { .. } => panic!("unexpected aax helper secret"),
        }
    }
}
