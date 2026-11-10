use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use abb_audible_core::{
    demuxer_options, file_matches_decrypt_kind, title_ref, AudibleDecryptionMaterial, DecryptKind,
    DemuxerOption,
};
use abb_remote_source_core::{acquisition_progress, AcquisitionProgress, AcquisitionStage};
use secrecy::ExposeSecret;
use tokio::task::JoinHandle;

use super::protected_file_probe::read_protected_file_facts;
use crate::engine_tasks::EngineTasks;
use crate::errors::{AppError, Result};
use crate::remote_source::cancellation::{ensure_not_cancelled, remote_acquisition_cancelled};
use crate::remote_source::scoped_output::{remove_logging_failure, StagedTempFile};

const POLL_INTERVAL: Duration = Duration::from_millis(100);

pub(super) struct DecryptRequest<'a> {
    pub(super) input: &'a Path,
    pub(super) output: &'a Path,
    pub(super) material: &'a AudibleDecryptionMaterial,
    pub(super) job_id: &'a str,
    pub(super) title_id: &'a str,
    pub(super) tasks: &'a EngineTasks,
}

#[derive(Clone, Default)]
pub(super) struct CopyControl {
    stop: Arc<AtomicBool>,
    fraction_bits: Arc<AtomicU32>,
}

impl CopyControl {
    pub(super) fn request_stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }

    fn stop_requested(&self) -> bool {
        self.stop.load(Ordering::Relaxed)
    }

    fn set_fraction(&self, fraction: f32) {
        self.fraction_bits
            .store(fraction.to_bits(), Ordering::Relaxed);
    }

    pub(super) fn fraction(&self) -> f32 {
        f32::from_bits(self.fraction_bits.load(Ordering::Relaxed))
    }
}

struct StopOnDrop(CopyControl);

impl Drop for StopOnDrop {
    fn drop(&mut self) {
        self.0.request_stop();
    }
}

struct CopyJob {
    job_id: String,
    title_ref: String,
    input: PathBuf,
    partial: PathBuf,
    kind: DecryptKind,
    options: Vec<DemuxerOption>,
    control: CopyControl,
}

/// Decrypts an Audible AAX or AAXC download into an MP4 by stream copy. Packet
/// success does not prove the key: AAXC decrypts to noise under a wrong key,
/// so the partial must decode as audio and read back its chapters before it
/// is committed to its final path.
pub(super) async fn decrypt_download(
    request: DecryptRequest<'_>,
    progress: &mut impl FnMut(AcquisitionProgress),
    is_cancelled: &impl Fn() -> bool,
) -> Result<PathBuf> {
    ensure_not_cancelled(is_cancelled)?;
    let mut staged = StagedTempFile::new(request.output);
    staged.prepare()?;
    let control = CopyControl::default();
    let _stop_on_drop = StopOnDrop(control.clone());
    let job = CopyJob {
        job_id: request.job_id.to_string(),
        title_ref: title_ref(request.title_id),
        input: request.input.to_path_buf(),
        partial: staged.partial_path().to_path_buf(),
        kind: request.material.kind(),
        options: demuxer_options(request.material),
        control: control.clone(),
    };
    let worker = request.tasks.spawn_blocking(move || copy_decrypted(job));
    await_copy(worker, &control, progress, is_cancelled).await?;
    staged.rename_and_commit(is_cancelled).await?;
    Ok(request.output.to_path_buf())
}

async fn await_copy(
    mut worker: JoinHandle<Result<()>>,
    control: &CopyControl,
    progress: &mut impl FnMut(AcquisitionProgress),
    is_cancelled: &impl Fn() -> bool,
) -> Result<()> {
    let mut ticker = tokio::time::interval(POLL_INTERVAL);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut reported = 0.0_f32;
    loop {
        tokio::select! {
            biased;
            _ = ticker.tick() => {
                if is_cancelled() {
                    control.request_stop();
                    if let Err(error) = (&mut worker).await {
                        log::warn!("remote_source audible stage=decrypt_stop_join_failed: {error}");
                    }
                    return Err(remote_acquisition_cancelled());
                }
                let fraction = control.fraction();
                if fraction > reported {
                    reported = fraction;
                    progress(acquisition_progress(
                        AcquisitionStage::Decryption,
                        Some(fraction),
                        None,
                        None,
                    ));
                }
            }
            joined = &mut worker => return joined.map_err(copy_task_failure)?,
        }
    }
}

fn copy_task_failure(error: tokio::task::JoinError) -> AppError {
    AppError::General(format!("Audible decrypt task failed: {error}"))
}

fn decrypt_failure(step: &str, error: impl std::fmt::Display) -> AppError {
    AppError::General(format!("Audible decrypt failed while {step}: {error}"))
}

fn copy_decrypted(job: CopyJob) -> Result<()> {
    refuse_lane_mismatch(&job)?;
    stream_copy(&job.input, &job.partial, &job.options, &job.control)?;
    confirm_decodes_as_audio(&job)
}

fn without_file_name(message: &str, path: &Path) -> String {
    match path.file_name().and_then(|name| name.to_str()) {
        Some(name) if !name.is_empty() => message.replace(name, "<partial>"),
        _ => message.to_string(),
    }
}

fn confirm_decodes_as_audio(job: &CopyJob) -> Result<()> {
    let Err(rejection) = crate::audio::probe_builtin_audio_decoder(&job.partial) else {
        return Ok(());
    };
    log::warn!(
        "remote_source audible stage=decrypt_output_rejected job_id={} title_ref={} error={}",
        job.job_id,
        job.title_ref,
        without_file_name(&rejection.to_string(), &job.partial)
    );
    remove_logging_failure(&job.partial);
    Err(AppError::General(
        "Audible decrypt did not produce decodable audio. Provider-private details were withheld from UI and logs."
            .to_string(),
    ))
}

/// Removes its own partial when it fails or is stopped: an aborted caller may
/// remove the partial before this worker has created it.
pub(super) fn stream_copy(
    input: &Path,
    partial: &Path,
    options: &[DemuxerOption],
    control: &CopyControl,
) -> Result<()> {
    let result = write_copy(input, partial, options, control).and_then(|()| {
        if control.stop_requested() {
            Err(remote_acquisition_cancelled())
        } else {
            Ok(())
        }
    });
    if result.is_err() {
        remove_logging_failure(partial);
    }
    result
}

fn write_copy(
    input: &Path,
    partial: &Path,
    options: &[DemuxerOption],
    control: &CopyControl,
) -> Result<()> {
    let mut input = open_protected(input, options)?;
    let mut output = abb_ffmpeg::Output::create(partial, crate::audio::Muxer::Mp4.ffmpeg_name())
        .map_err(|error| decrypt_failure("creating the output", error))?;
    let mapping = copy_streams(&input, &mut output)?;
    let chapters = input.chapters();
    copy_chapters(&chapters, &mut output)?;
    output
        .set_tags(&input.tags())
        .map_err(|error| decrypt_failure("copying the tags", error))?;
    let mut muxer = crate::audio::write_layer_output_header(output)
        .map_err(|error| decrypt_failure("writing the output header", error))?;
    copy_packets(&mut input, &mut muxer, &mapping, control)?;
    muxer
        .finish()
        .map_err(|error| decrypt_failure("finishing the output", error))?;
    crate::metadata::verify_chapters(partial, &crate::metadata::layer_chapter_specs(&chapters))
        .map_err(|error| decrypt_failure("checking the chapters", error))
}

fn refuse_lane_mismatch(job: &CopyJob) -> Result<()> {
    let facts = read_protected_file_facts(&job.input)
        .map_err(|error| decrypt_failure("reading the download's format", error))?;
    log::info!(
        "remote_source audible stage=protected_file_facts job_id={} title_ref={} lane={:?} major_brand={:?} compatible_brands={:?} drm_boxes={:?}",
        job.job_id,
        job.title_ref,
        job.kind,
        facts.major_brand,
        facts.compatible_brands,
        facts.drm_boxes
    );
    if file_matches_decrypt_kind(&facts, job.kind) {
        return Ok(());
    }
    log::warn!(
        "remote_source audible stage=decrypt_lane_mismatch job_id={} title_ref={} lane={:?}",
        job.job_id,
        job.title_ref,
        job.kind
    );
    Err(AppError::General(format!(
        "Audible decrypt refused the download: its format does not match the {:?} license.",
        job.kind
    )))
}

fn open_protected(input: &Path, options: &[DemuxerOption]) -> Result<abb_ffmpeg::Input> {
    let mut secret = abb_ffmpeg::SecretOptions::default();
    for option in options {
        secret
            .push(option.name, option.value.expose_secret())
            .map_err(|error| decrypt_failure("opening the download", error))?;
    }
    let opened = abb_ffmpeg::Input::open(input, &secret)
        .map_err(|error| decrypt_failure("opening the download", error))?;
    if opened.unused_options.is_empty() {
        Ok(opened.input)
    } else {
        Err(decrypt_failure(
            "opening the download",
            format!(
                "FFmpeg did not take the options {}",
                opened.unused_options.join(", ")
            ),
        ))
    }
}

fn is_copied(stream: &abb_ffmpeg::StreamFacts) -> bool {
    match stream.medium {
        abb_ffmpeg::Medium::Audio => true,
        abb_ffmpeg::Medium::Video => stream
            .disposition
            .contains(abb_ffmpeg::Disposition::ATTACHED_PIC),
        abb_ffmpeg::Medium::Attachment | abb_ffmpeg::Medium::Other => false,
    }
}

fn copy_streams(
    input: &abb_ffmpeg::Input,
    output: &mut abb_ffmpeg::Output,
) -> Result<Vec<Option<usize>>> {
    let streams = input.streams();
    let mut mapping = vec![None; streams.len()];
    for istream in streams.iter().filter(|stream| is_copied(stream)) {
        let index = output
            .add_stream_copied_from(input, istream.index)
            .map_err(|error| decrypt_failure("creating an output stream", error))?;
        output
            .set_stream_time_base(index, istream.time_base)
            .and_then(|()| output.set_stream_tags(index, &istream.tags))
            .and_then(|()| {
                output.set_stream_disposition_and_clear_codec_tag(index, istream.disposition)
            })
            .map_err(|error| decrypt_failure("creating an output stream", error))?;
        mapping[istream.index] = Some(index);
    }
    Ok(mapping)
}

/// Every copied chapter carries a title tag; a chapter without one gets an
/// empty title.
fn copy_chapters(chapters: &[abb_ffmpeg::Chapter], output: &mut abb_ffmpeg::Output) -> Result<()> {
    for chapter in chapters {
        let failure = |error| decrypt_failure(&format!("copying chapter {}", chapter.id), error);
        let step = abb_ffmpeg::Step::AddChapter;
        let title = match chapter.title() {
            Some(title) => title.clone(),
            None => abb_ffmpeg::Text::new("", step).map_err(failure)?,
        };
        let mut tags = abb_ffmpeg::Tags::default();
        tags.insert(
            abb_ffmpeg::Text::new("title", step).map_err(failure)?,
            title,
        );
        output
            .add_chapter(&abb_ffmpeg::Chapter {
                tags,
                ..chapter.clone()
            })
            .map_err(failure)?;
    }
    Ok(())
}

fn copy_packets(
    input: &mut abb_ffmpeg::Input,
    muxer: &mut abb_ffmpeg::Muxer,
    mapping: &[Option<usize>],
    control: &CopyControl,
) -> Result<()> {
    let duration_seconds = input.duration_micros() as f64 / 1_000_000.0;
    let time_bases: Vec<abb_ffmpeg::Rational> = input
        .streams()
        .iter()
        .map(|stream| stream.time_base)
        .collect();
    let mut packet =
        abb_ffmpeg::Packet::new().map_err(|error| decrypt_failure("reading audio", error))?;
    while next_packet(input, &mut packet, control)? {
        let Some(out_index) = mapping.get(packet.stream_index()).copied().flatten() else {
            continue;
        };
        let in_base = time_bases
            .get(packet.stream_index())
            .copied()
            .ok_or_else(|| decrypt_failure("reading audio", "a packet names a missing stream"))?;
        if let Some(fraction) = packet_fraction(packet.pts(), in_base, duration_seconds) {
            control.set_fraction(fraction);
        }
        let out_base = muxer
            .stream_time_base(out_index)
            .map_err(|error| decrypt_failure("writing audio", error))?;
        packet.set_stream_index(out_index);
        packet.rescale_ts(in_base, out_base);
        muxer
            .write_interleaved(&mut packet)
            .map_err(|error| decrypt_failure("writing audio", error))?;
    }
    Ok(())
}

fn next_packet(
    input: &mut abb_ffmpeg::Input,
    packet: &mut abb_ffmpeg::Packet,
    control: &CopyControl,
) -> Result<bool> {
    if control.stop_requested() {
        return Err(remote_acquisition_cancelled());
    }
    input
        .read_packet(packet)
        .map_err(|error| decrypt_failure("reading audio", error))
}

/// How far a packet's time is through the book, or `None` when either is
/// unknown.
fn packet_fraction(
    pts: Option<i64>,
    time_base: abb_ffmpeg::Rational,
    duration_seconds: f64,
) -> Option<f32> {
    if duration_seconds <= 0.0 || time_base.denominator == 0 {
        return None;
    }
    let seconds = pts? as f64 * f64::from(time_base.numerator) / f64::from(time_base.denominator);
    Some((seconds / duration_seconds).clamp(0.0, 1.0) as f32)
}

#[cfg(test)]
#[path = "decrypt_tests.rs"]
mod tests;
