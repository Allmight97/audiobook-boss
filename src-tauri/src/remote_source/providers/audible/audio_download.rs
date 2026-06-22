use std::fs;
use std::path::Path;

use super::acquisition::{ensure_not_cancelled, with_title_progress, TitleAcquisitionCtx};
use super::http::{audio_download_client, stream_response_chunks};
use super::{
    provider_private_failure, AUDIBLE_DOWNLOAD_USER_AGENT, MAX_DOWNLOAD_ATTEMPTS,
    MAX_DOWNLOAD_REDIRECTS,
};
use crate::errors::{AppError, Result};
use crate::remote_source::scoped_output::{rollback_committed_file, StagedTempFile};
use abb_audible_core::{classify_download_response_for_mode, title_ref, DownloadResponseError};
use abb_remote_source_core::{acquisition_progress, AcquisitionProgress, AcquisitionStage};
use reqwest::header::{CONTENT_RANGE, RANGE, USER_AGENT};

#[derive(Clone, Copy)]
pub(super) struct DownloadLogContext<'a> {
    job_id: &'a str,
    title_id: &'a str,
    extension: &'a str,
}

fn download_failure(stage: &str) -> AppError {
    AppError::General(format!(
        "Audible download {stage} failed. Provider-private details were withheld from UI and logs."
    ))
}

pub(super) fn download_status_failure(status: u16) -> AppError {
    AppError::General(format!(
        "Remote source download returned HTTP {status}. Check application logs for sanitized acquisition facts."
    ))
}

fn map_download_response_error(error: DownloadResponseError) -> AppError {
    match error {
        DownloadResponseError::RedirectNotHttps => {
            AppError::InvalidInput("Remote source download redirect must use https.".to_string())
        }
        DownloadResponseError::ContentRange => download_failure("content range"),
        DownloadResponseError::UnexpectedStatus(status) => download_status_failure(status),
    }
}

pub(super) async fn download_audio(
    content_url: &str,
    path: &Path,
    ctx: TitleAcquisitionCtx<'_>,
    progress: &mut impl FnMut(AcquisitionProgress),
    is_cancelled: &impl Fn() -> bool,
) -> Result<()> {
    let TitleAcquisitionCtx {
        job_id,
        title_id,
        progress_context,
        ..
    } = ctx;
    ensure_not_cancelled(is_cancelled)?;
    let extension = path
        .extension()
        .map(|value| value.to_string_lossy())
        .unwrap_or_else(|| std::borrow::Cow::Borrowed("bin"));
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    log::info!(
        "remote_source audible stage=download_start job_id={} title_ref={} extension={}",
        job_id,
        title_ref(title_id),
        extension
    );
    let mut download_progress = |progress_event: AcquisitionProgress| {
        progress(with_title_progress(progress_event, progress_context));
    };
    let bytes = download_to_path(
        content_url,
        path,
        Some(DownloadLogContext {
            job_id,
            title_id,
            extension: &extension,
        }),
        &mut download_progress,
        is_cancelled,
    )
    .await?;
    if let Err(error @ AppError::Cancellation(_)) = ensure_not_cancelled(is_cancelled) {
        rollback_committed_file(path)?;
        return Err(error);
    }
    log::info!(
        "remote_source audible stage=download_complete job_id={} title_ref={} extension={} bytes={}",
        job_id,
        title_ref(title_id),
        extension,
        bytes
    );
    Ok(())
}

pub(super) async fn download_to_path(
    url: &str,
    path: &Path,
    log_context: Option<DownloadLogContext<'_>>,
    progress: &mut impl FnMut(AcquisitionProgress),
    is_cancelled: &impl Fn() -> bool,
) -> Result<u64> {
    ensure_not_cancelled(is_cancelled)?;
    let parsed = reqwest::Url::parse(url).map_err(|_| provider_private_failure("download URL"))?;
    if parsed.scheme() != "https" {
        return Err(AppError::InvalidInput(
            "Remote source download URL must use https.".to_string(),
        ));
    }

    let mut staged = StagedTempFile::new(path);
    staged.prepare()?;
    let bytes = download_to_partial_path(
        parsed,
        staged.partial_path(),
        log_context,
        progress,
        is_cancelled,
    )
    .await?;
    staged.rename_and_commit(is_cancelled).await?;
    Ok(bytes)
}

async fn download_to_partial_path(
    url: reqwest::Url,
    path: &Path,
    log_context: Option<DownloadLogContext<'_>>,
    progress: &mut impl FnMut(AcquisitionProgress),
    is_cancelled: &impl Fn() -> bool,
) -> Result<u64> {
    ensure_not_cancelled(is_cancelled)?;
    let client = audio_download_client(MAX_DOWNLOAD_REDIRECTS)
        .map_err(|_| provider_private_failure("download client"))?;
    let mut state = DownloadProgress::default();
    let can_resume = log_context.is_some();

    for attempt in 0..MAX_DOWNLOAD_ATTEMPTS {
        ensure_not_cancelled(is_cancelled)?;
        let outcome = run_download_attempt(
            &client,
            &url,
            path,
            &mut state,
            log_context,
            progress,
            is_cancelled,
        )
        .await?;
        match outcome {
            AttemptOutcome::Complete => return Ok(state.bytes_downloaded),
            AttemptOutcome::ReadFailed => {
                if can_resume && attempt + 1 < MAX_DOWNLOAD_ATTEMPTS {
                    continue;
                }
                log_download_failed(
                    log_context,
                    "read",
                    state.bytes_downloaded,
                    state.bytes_total,
                    None,
                );
                return Err(download_failure("read"));
            }
            AttemptOutcome::Incomplete => {
                if can_resume && attempt + 1 < MAX_DOWNLOAD_ATTEMPTS {
                    continue;
                }
                break;
            }
        }
    }

    log_download_failed(
        log_context,
        "incomplete",
        state.bytes_downloaded,
        state.bytes_total,
        None,
    );
    Err(download_failure("incomplete"))
}

#[derive(Default)]
struct DownloadProgress {
    bytes_downloaded: u64,
    bytes_total: Option<u64>,
    first_bytes_logged: bool,
}

enum AttemptOutcome {
    Complete,
    ReadFailed,
    Incomplete,
}

/// Run a single download attempt: send the (optionally ranged) request, classify
/// the response, then stream the body into `path`. Returns the attempt outcome;
/// `Err` is reserved for terminal failures (request/status/IO) that must not be
/// retried.
async fn run_download_attempt(
    client: &reqwest::Client,
    url: &reqwest::Url,
    path: &Path,
    state: &mut DownloadProgress,
    log_context: Option<DownloadLogContext<'_>>,
    progress: &mut impl FnMut(AcquisitionProgress),
    is_cancelled: &impl Fn() -> bool,
) -> Result<AttemptOutcome> {
    log_download_request_start(log_context, state.bytes_downloaded);
    let request = if log_context.is_some() {
        build_download_request(client, url.clone(), state.bytes_downloaded)
    } else {
        client.get(url.clone())
    };
    let mut response = match request.send().await {
        Ok(response) => response,
        Err(_) => {
            log_download_failed(
                log_context,
                "request",
                state.bytes_downloaded,
                state.bytes_total,
                None,
            );
            return Err(download_failure("request"));
        }
    };
    ensure_not_cancelled(is_cancelled)?;

    let status = response.status();
    let content_range = response
        .headers()
        .get(CONTENT_RANGE)
        .and_then(|value| value.to_str().ok());
    let final_url_is_https = response.url().scheme() == "https";
    let response_total = match classify_download_response_for_mode(
        log_context.is_some(),
        status.as_u16(),
        final_url_is_https,
        state.bytes_downloaded,
        response.content_length(),
        content_range,
    ) {
        Ok(total) => total,
        Err(error) => {
            log_download_failed(
                log_context,
                "status",
                state.bytes_downloaded,
                state.bytes_total,
                Some(status),
            );
            return Err(map_download_response_error(error));
        }
    };
    state.bytes_total = response_total.or(state.bytes_total);
    log_download_request_status(
        log_context,
        status,
        state.bytes_downloaded,
        state.bytes_total,
    );

    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .await?;
    let read_failed = stream_download_chunks(
        &mut response,
        &mut file,
        state,
        log_context,
        progress,
        is_cancelled,
    )
    .await?;
    file.sync_all().await?;
    if read_failed {
        return Ok(AttemptOutcome::ReadFailed);
    }
    ensure_not_cancelled(is_cancelled)?;
    if state
        .bytes_total
        .is_none_or(|total| state.bytes_downloaded >= total)
        && state.bytes_downloaded > 0
    {
        return Ok(AttemptOutcome::Complete);
    }
    Ok(AttemptOutcome::Incomplete)
}

/// Stream one HTTP response body into the append-mode `file`, updating progress
/// and the running byte counters. Returns `true` if the body read failed midway
/// (a resumable condition handled by the caller's retry loop).
async fn stream_download_chunks(
    response: &mut reqwest::Response,
    file: &mut tokio::fs::File,
    state: &mut DownloadProgress,
    log_context: Option<DownloadLogContext<'_>>,
    progress: &mut impl FnMut(AcquisitionProgress),
    is_cancelled: &impl Fn() -> bool,
) -> Result<bool> {
    stream_response_chunks(response, file, is_cancelled, |chunk| {
        state.bytes_downloaded += chunk.len() as u64;
        if !state.first_bytes_logged {
            state.first_bytes_logged = true;
            log_download_progress_first_bytes(
                log_context,
                state.bytes_downloaded,
                state.bytes_total,
            );
        }
        let fraction = state
            .bytes_total
            .filter(|total| *total > 0)
            .map(|total| state.bytes_downloaded as f32 / total as f32)
            .unwrap_or(0.2);
        progress(acquisition_progress(
            AcquisitionStage::Download,
            Some(fraction),
            Some(state.bytes_downloaded),
            state.bytes_total,
        ));
        Ok(())
    })
    .await
}

pub(super) fn build_download_request(
    client: &reqwest::Client,
    url: reqwest::Url,
    offset: u64,
) -> reqwest::RequestBuilder {
    client
        .get(url)
        .header(USER_AGENT, AUDIBLE_DOWNLOAD_USER_AGENT)
        .header(RANGE, format!("bytes={offset}-"))
}

fn log_download_request_start(context: Option<DownloadLogContext<'_>>, offset: u64) {
    let Some(context) = context else {
        return;
    };
    log::info!(
        "remote_source audible stage=download_request_start job_id={} title_ref={} extension={} bytes={}",
        context.job_id,
        title_ref(context.title_id),
        context.extension,
        offset
    );
}

fn log_download_request_status(
    context: Option<DownloadLogContext<'_>>,
    status: reqwest::StatusCode,
    bytes_downloaded: u64,
    bytes_total: Option<u64>,
) {
    let Some(context) = context else {
        return;
    };
    log::info!(
        "remote_source audible stage=download_request_status job_id={} title_ref={} extension={} http_status={} bytes={} bytes_total={}",
        context.job_id,
        title_ref(context.title_id),
        context.extension,
        status.as_u16(),
        bytes_downloaded,
        bytes_total.unwrap_or(0)
    );
}

fn log_download_progress_first_bytes(
    context: Option<DownloadLogContext<'_>>,
    bytes_downloaded: u64,
    bytes_total: Option<u64>,
) {
    let Some(context) = context else {
        return;
    };
    log::info!(
        "remote_source audible stage=download_progress_first_bytes job_id={} title_ref={} extension={} bytes={} bytes_total={}",
        context.job_id,
        title_ref(context.title_id),
        context.extension,
        bytes_downloaded,
        bytes_total.unwrap_or(0)
    );
}

fn log_download_failed(
    context: Option<DownloadLogContext<'_>>,
    category: &str,
    bytes_downloaded: u64,
    bytes_total: Option<u64>,
    status: Option<reqwest::StatusCode>,
) {
    let Some(context) = context else {
        return;
    };
    log::warn!(
        "remote_source audible stage=download_failed job_id={} title_ref={} extension={} category={} http_status={} bytes={} bytes_total={}",
        context.job_id,
        title_ref(context.title_id),
        context.extension,
        category,
        status.map(|status| status.as_u16()).unwrap_or(0),
        bytes_downloaded,
        bytes_total.unwrap_or(0)
    );
}


