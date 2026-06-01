use std::fs;
use std::path::Path;

use audible_api::api::Client as AudibleClient;
use audible_api::auth::oauth::{build_oauth_url, extract_auth_code};
use audible_api::auth::register::register;
use audible_api::auth::{localization, Auth};
use secrecy::{ExposeSecret, SecretString};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;

mod library;

use library::parse_library_titles;

use crate::audio;
use crate::errors::{sanitize_path_for_display, AppError, Result};
use crate::remote_source::vault::SecretVault;
use crate::remote_source::{
    AccountRef, AcquisitionJob, AcquisitionPlan, MaterializedSourceFile, ProviderId,
    RemoteAccountStatus, RemoteAcquisitionFailureKind, RemoteAcquisitionStatus, RemoteAuthFlow,
    RemoteLibraryResponse, RemoteSourceAccountState, RemoteSourceDiagnostic,
    RemoteSourceProviderCapabilities, SupplementalAsset,
};

const COUNTRY_CODE: &str = "us";
const DOMAIN: &str = "com";
const MARKETPLACE_ID: &str = "ATVPDKIKX0DER";
const ACCOUNT_ID: &str = "audible-us";
const AUTH_SECRET_KEY: &str = "audible.us.auth";
const MAX_SUPPLEMENTAL_PDF_BYTES: u64 = 100 * 1024 * 1024;

struct AcquiredTitle {
    file: Option<MaterializedSourceFile>,
    assets: Vec<SupplementalAsset>,
    diagnostics: Vec<RemoteSourceDiagnostic>,
}

fn provider_private_failure(stage: &str) -> AppError {
    AppError::General(format!(
        "Audible {stage} failed. Provider-private details were withheld from UI and logs."
    ))
}

#[derive(Debug, Clone)]
pub(in crate::remote_source) struct PendingAudibleAuth {
    pub(super) code_verifier: String,
    pub(super) device_serial: String,
}

#[derive(Debug, Clone)]
pub(in crate::remote_source) struct AudibleProvider;

impl AudibleProvider {
    pub(in crate::remote_source) fn capabilities() -> RemoteSourceProviderCapabilities {
        RemoteSourceProviderCapabilities {
            provider_id: ProviderId::Audible,
            label: "Audible".to_string(),
            auth_flow: RemoteAuthFlow::ExternalBrowserHandoff,
            supports_library_scan: true,
            supports_paged_scan: true,
            supports_typeahead_filter: true,
            supports_supplemental_pdf: true,
            supports_materialized_audio: true,
            supports_refresh: true,
            requires_live_session: true,
            known_unsupported_reasons: vec![
                RemoteAcquisitionFailureKind::ProviderPrivateProtocolFailed,
                RemoteAcquisitionFailureKind::ProtectedUnsupported,
            ],
        }
    }

    pub(in crate::remote_source) fn start_auth() -> Result<(String, PendingAudibleAuth)> {
        let (url, code_verifier, device_serial) =
            build_oauth_url(COUNTRY_CODE, DOMAIN, MARKETPLACE_ID, None, false).map_err(
                |error| AppError::General(format!("Failed to start Audible auth flow: {error}")),
            )?;
        Ok((
            url,
            PendingAudibleAuth {
                code_verifier,
                device_serial,
            },
        ))
    }

    pub(in crate::remote_source) async fn complete_auth(
        vault: &dyn SecretVault,
        pending: PendingAudibleAuth,
        response_url: &str,
    ) -> Result<RemoteSourceAccountState> {
        let authorization_code = extract_auth_code(response_url).map_err(|error| {
            AppError::InvalidInput(format!(
                "Audible auth response URL was not accepted: {error}"
            ))
        })?;
        let registration = register(
            &authorization_code,
            &pending.code_verifier,
            DOMAIN,
            &pending.device_serial,
            false,
        )
        .await
        .map_err(|_| provider_private_failure("device registration"))?;
        let locale = localization::find_by_country_code(COUNTRY_CODE)
            .ok_or_else(|| AppError::General("Audible locale is unavailable".to_string()))?;
        let auth = Auth {
            locale,
            device_registration: registration,
            authorization_code,
            code_verifier: pending.code_verifier,
        };
        let serialized = serde_json::to_string(&auth).map_err(|error| {
            AppError::General(format!("Failed to serialize Audible auth: {error}"))
        })?;
        vault.set_secret(AUTH_SECRET_KEY, SecretString::from(serialized))?;
        Self::account_state(vault)
    }

    pub(in crate::remote_source) fn account_state(
        vault: &dyn SecretVault,
    ) -> Result<RemoteSourceAccountState> {
        let Some(secret) = vault.get_secret(AUTH_SECRET_KEY)? else {
            return Ok(RemoteSourceAccountState {
                provider_id: ProviderId::Audible,
                status: RemoteAccountStatus::NeedsAuth,
                account: None,
                message: Some("Connect Audible to load your library.".to_string()),
            });
        };

        let auth: Auth = serde_json::from_str(secret.expose_secret()).map_err(|error| {
            AppError::ResourceCleanup(format!("Stored Audible auth is invalid: {error}"))
        })?;
        let display_name = auth
            .device_registration
            .customer_info
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("Audible account")
            .to_string();

        Ok(RemoteSourceAccountState {
            provider_id: ProviderId::Audible,
            status: RemoteAccountStatus::Connected,
            account: Some(AccountRef {
                provider_id: ProviderId::Audible,
                account_id: ACCOUNT_ID.to_string(),
                display_name,
            }),
            message: None,
        })
    }

    pub(in crate::remote_source) fn logout(vault: &dyn SecretVault) -> Result<()> {
        vault.delete_secret(AUTH_SECRET_KEY)
    }

    pub(in crate::remote_source) async fn load_library(
        vault: &dyn SecretVault,
    ) -> Result<RemoteLibraryResponse> {
        let client = client_from_vault(vault)?;
        let payload = client
            .get_library(Some(json!({
                "num_results": 100,
                "response_groups": "product_desc,product_attrs,contributors,media,pdf_url,product_details",
                "sort_by": "-PurchaseDate"
            })))
            .await
            .map_err(|_| provider_private_failure("library request"))?;
        let titles = parse_library_titles(&payload);
        Ok(RemoteLibraryResponse {
            provider_id: ProviderId::Audible,
            titles,
            diagnostics: Vec::new(),
        })
    }

    pub(in crate::remote_source) async fn acquire(
        vault: &dyn SecretVault,
        plan: &AcquisitionPlan,
        job_id: &str,
        job_dir: &Path,
    ) -> Result<AcquisitionJob> {
        let client = client_from_vault(vault)?;
        let mut job = AcquisitionJob {
            job_id: job_id.to_string(),
            provider_id: ProviderId::Audible,
            status: RemoteAcquisitionStatus::Acquiring,
            materialized_files: Vec::new(),
            supplemental_assets: Vec::new(),
            diagnostics: Vec::new(),
        };

        for selection in &plan.selections {
            match acquire_one(
                &client,
                &selection.title_id,
                selection.include_supplemental_pdf,
                job_dir,
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
                    job.diagnostics.push(RemoteSourceDiagnostic {
                        kind: RemoteAcquisitionFailureKind::MaterializationFailed,
                        title_id: Some(selection.title_id.clone()),
                        message: error.to_string(),
                    });
                }
            }
        }

        job.status = if job.materialized_files.is_empty() {
            RemoteAcquisitionStatus::Failed
        } else {
            RemoteAcquisitionStatus::Validated
        };
        Ok(job)
    }
}

fn client_from_vault(vault: &dyn SecretVault) -> Result<AudibleClient> {
    let secret = vault
        .get_secret(AUTH_SECRET_KEY)?
        .ok_or_else(|| AppError::InvalidInput("Audible account is not connected.".to_string()))?;
    let auth: Auth = serde_json::from_str(secret.expose_secret()).map_err(|error| {
        AppError::ResourceCleanup(format!("Stored Audible auth is invalid: {error}"))
    })?;
    AudibleClient::new(auth)
        .map_err(|error| AppError::General(format!("Failed to create Audible client: {error}")))
}

async fn acquire_one(
    client: &AudibleClient,
    title_id: &str,
    include_pdf: bool,
    job_dir: &Path,
) -> Result<AcquiredTitle> {
    let metadata = client
        .get_library_item_by_asin(
            title_id,
            Some(json!({
                "response_groups": "product_desc,product_attrs,contributors,media,pdf_url,product_details"
            })),
        )
        .await
        .map_err(|_| provider_private_failure("title lookup"))?;
    let supplemental_pdf_url = include_pdf
        .then(|| {
            find_first_string_for_key(&metadata, "pdf_url")
                .or_else(|| find_first_string_for_key(&metadata, "pdfUrl"))
        })
        .flatten();

    let license = client
        .post_license_request(
            title_id,
            Some(json!({
                "quality": "High",
                "response_groups": "chapter_info,content_reference,last_position_heard,pdf_url,ad_insertion,certificate",
                "consumption_type": "Download",
                "supported_media_features": {
                    "codecs": ["mp4a.40.2", "mp4a.40.42"],
                    "drm_types": ["Mpeg"]
                },
                "spatial": false
            })),
        )
        .await
        .map_err(|_| provider_private_failure("license/materialization request"))?;
    let Some(content_url) = find_first_string_for_key(&license, "content_url")
        .or_else(|| find_first_string_for_key(&license, "contentUrl"))
    else {
        return Ok(AcquiredTitle {
            file: None,
            assets: Vec::new(),
            diagnostics: Vec::new(),
        });
    };

    let file = download_and_validate_audio(title_id, &content_url, job_dir).await?;
    let mut assets = Vec::new();
    let mut diagnostics = Vec::new();
    if let Some(pdf_url) = supplemental_pdf_url {
        match download_pdf(title_id, &file.input_id, &pdf_url, job_dir).await {
            Ok(asset) => assets.push(asset),
            Err(error) => diagnostics.push(RemoteSourceDiagnostic {
                kind: RemoteAcquisitionFailureKind::SupplementalPdfFailed,
                title_id: Some(title_id.to_string()),
                message: error.to_string(),
            }),
        }
    }
    Ok(AcquiredTitle {
        file: Some(file),
        assets,
        diagnostics,
    })
}

async fn download_and_validate_audio(
    title_id: &str,
    content_url: &str,
    job_dir: &Path,
) -> Result<MaterializedSourceFile> {
    fs::create_dir_all(job_dir)?;
    let path = generated_staging_path(job_dir, "m4b");
    download_to_path(content_url, &path).await?;
    let metadata = fs::metadata(&path)?;
    let sha256 = sha256_file(&path)?;

    match audio::get_file_list_info(std::slice::from_ref(&path)) {
        Ok(info) if info.valid_count == 1 => {
            let accepted_file = &info.files[0];
            Ok(MaterializedSourceFile {
                input_id: accepted_file.input_id.clone(),
                title_id: title_id.to_string(),
                path: accepted_file.path.clone(),
                size_bytes: metadata.len(),
                sha256,
            })
        }
        Ok(_) => Err(AppError::FileValidation(format!(
            "Materialized Audible file was not accepted as audio: {}",
            sanitize_path_for_display(&path)
        ))),
        Err(error) => Err(error),
    }
}

async fn download_pdf(
    title_id: &str,
    input_id: &str,
    url: &str,
    job_dir: &Path,
) -> Result<SupplementalAsset> {
    fs::create_dir_all(job_dir)?;
    let path = generated_staging_path(job_dir, "pdf");
    download_to_path(url, &path).await?;
    let bytes = fs::read(&path)?;
    if !bytes.starts_with(b"%PDF-") {
        return Err(AppError::FileValidation(
            "Downloaded Supplemental PDF did not pass PDF magic-byte validation.".to_string(),
        ));
    }
    let metadata = fs::metadata(&path)?;
    if metadata.len() > MAX_SUPPLEMENTAL_PDF_BYTES {
        return Err(AppError::FileValidation(
            "Downloaded Supplemental PDF exceeds the 100 MiB size limit.".to_string(),
        ));
    }
    let path = path.canonicalize().map_err(|error| {
        AppError::FileValidation(format!(
            "Cannot canonicalize Supplemental PDF source '{}': {}",
            sanitize_path_for_display(&path),
            error
        ))
    })?;
    Ok(SupplementalAsset {
        asset_id: uuid::Uuid::new_v4().to_string(),
        input_id: input_id.to_string(),
        title_id: title_id.to_string(),
        path,
        file_name: "Supplemental PDF.pdf".to_string(),
        size_bytes: metadata.len(),
        sha256: sha256_bytes(&bytes),
    })
}

async fn download_to_path(url: &str, path: &Path) -> Result<()> {
    let parsed = reqwest::Url::parse(url).map_err(|_| provider_private_failure("download URL"))?;
    if !matches!(parsed.scheme(), "https" | "http") {
        return Err(AppError::InvalidInput(
            "Remote source download URL must use http or https.".to_string(),
        ));
    }

    let partial_path = partial_download_path(path);
    let result = download_to_partial_path(parsed, &partial_path).await;
    if let Err(error) = result {
        let _ = tokio::fs::remove_file(&partial_path).await;
        return Err(error);
    }
    tokio::fs::rename(&partial_path, path).await?;
    Ok(())
}

async fn download_to_partial_path(url: reqwest::Url, path: &Path) -> Result<()> {
    let response = reqwest::get(url)
        .await
        .map_err(|_| provider_private_failure("download request"))?;
    if !response.status().is_success() {
        return Err(AppError::General(format!(
            "Remote source download returned HTTP {}",
            response.status()
        )));
    }
    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .await?;
    let mut response = response;
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| provider_private_failure("download read"))?
    {
        file.write_all(&chunk).await?;
    }
    file.sync_all().await?;
    Ok(())
}

fn generated_staging_path(job_dir: &Path, extension: &str) -> std::path::PathBuf {
    job_dir.join(format!("{}.{}", uuid::Uuid::new_v4(), extension))
}

fn partial_download_path(path: &Path) -> std::path::PathBuf {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .map(|extension| format!("{extension}.partial"))
        .unwrap_or_else(|| "partial".to_string());
    path.with_extension(extension)
}

fn find_first_string_for_key(value: &Value, key: &str) -> Option<String> {
    match value {
        Value::Object(map) => {
            if let Some(found) = map.get(key).and_then(Value::as_str) {
                return Some(found.to_string());
            }
            map.values()
                .find_map(|entry| find_first_string_for_key(entry, key))
        }
        Value::Array(values) => values
            .iter()
            .find_map(|entry| find_first_string_for_key(entry, key)),
        _ => None,
    }
}

fn sha256_file(path: &Path) -> Result<String> {
    let bytes = fs::read(path)?;
    Ok(sha256_bytes(&bytes))
}

fn sha256_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capabilities_stay_provider_neutral() {
        let capabilities = AudibleProvider::capabilities();

        assert_eq!(capabilities.provider_id, ProviderId::Audible);
        assert_eq!(
            capabilities.auth_flow,
            RemoteAuthFlow::ExternalBrowserHandoff
        );
        assert!(capabilities.supports_library_scan);
        assert!(capabilities.supports_supplemental_pdf);
        assert!(capabilities.supports_materialized_audio);
        assert!(capabilities
            .known_unsupported_reasons
            .contains(&RemoteAcquisitionFailureKind::ProtectedUnsupported));
    }

    #[test]
    fn pdf_hash_uses_sha256_hex() {
        assert_eq!(
            sha256_bytes(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

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
    fn partial_download_path_keeps_final_extension_visible() {
        let path = Path::new("/tmp/book.m4b");

        assert_eq!(
            partial_download_path(path),
            Path::new("/tmp/book.m4b.partial")
        );
    }
}
