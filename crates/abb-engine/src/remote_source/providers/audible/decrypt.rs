use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use abb_audible_core::{
    demuxer_options, file_matches_decrypt_kind, title_ref, AudibleDecryptionMaterial, DecryptKind,
    DemuxerOption,
};
use abb_remote_source_core::{acquisition_progress, AcquisitionProgress, AcquisitionStage};
use ffmpeg_next as ff;
use secrecy::ExposeSecret;
use tokio::task::JoinHandle;

use super::protected_file_probe::read_protected_file_facts;
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
    #[expect(
        clippy::disallowed_methods,
        reason = "joined by await_copy, or stopped by StopOnDrop if the caller is aborted"
    )]
    let worker = tokio::task::spawn_blocking(move || copy_decrypted(job));
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

fn confirm_decodes_as_audio(job: &CopyJob) -> Result<()> {
    if crate::audio::probe_builtin_audio_decoder(&job.partial).is_ok() {
        return Ok(());
    }
    log::warn!(
        "remote_source audible stage=decrypt_output_rejected job_id={} title_ref={}",
        job.job_id,
        job.title_ref
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
    ff::init().map_err(|error| decrypt_failure("starting FFmpeg", error))?;
    let mut ictx = open_protected(input, options)?;
    let mut octx = ff::format::output_as(partial, crate::audio::Muxer::Mp4.ffmpeg_name())
        .map_err(|error| decrypt_failure("creating the output", error))?;
    let mapping = copy_streams(&ictx, &mut octx)?;
    copy_chapters(&ictx, &mut octx)?;
    octx.set_metadata(ictx.metadata().to_owned());
    crate::audio::write_output_header(&mut octx, ff::Dictionary::new())
        .map_err(|error| decrypt_failure("writing the output header", error))?;
    copy_packets(&mut ictx, &mut octx, &mapping, control)?;
    octx.write_trailer()
        .map_err(|error| decrypt_failure("finishing the output", error))?;
    drop(octx);
    crate::metadata::verify_chapters(partial, &crate::metadata::read_chapters(&ictx))
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

fn open_protected(input: &Path, options: &[DemuxerOption]) -> Result<ff::format::context::Input> {
    let mut dictionary = ff::Dictionary::new();
    for option in options {
        dictionary.set(option.name, option.value.expose_secret());
    }
    ff::format::input_with_dictionary(input, dictionary)
        .map_err(|error| decrypt_failure("opening the download", error))
}

fn is_copied(stream: &ff::format::stream::Stream) -> bool {
    match stream.parameters().medium() {
        ff::media::Type::Audio => true,
        ff::media::Type::Video => stream
            .disposition()
            .contains(ff::format::stream::Disposition::ATTACHED_PIC),
        _ => false,
    }
}

fn copy_streams(
    ictx: &ff::format::context::Input,
    octx: &mut ff::format::context::Output,
) -> Result<Vec<Option<usize>>> {
    let mut mapping = vec![None; ictx.streams().len()];
    for istream in ictx.streams().filter(is_copied) {
        let codec = ff::codec::context::Context::from_parameters(istream.parameters())
            .map_err(|error| decrypt_failure("reading stream parameters", error))?;
        let mut ostream = octx
            .add_stream_with(&codec)
            .map_err(|error| decrypt_failure("creating an output stream", error))?;
        ostream.set_time_base(istream.time_base());
        ostream.set_metadata(istream.metadata().to_owned());
        crate::metadata::set_stream_disposition_and_clear_codec_tag(
            &mut ostream,
            istream.disposition(),
        );
        mapping[istream.index()] = Some(ostream.index());
    }
    Ok(mapping)
}

fn copy_chapters(
    ictx: &ff::format::context::Input,
    octx: &mut ff::format::context::Output,
) -> Result<()> {
    for chapter in ictx.chapters() {
        let title = chapter.metadata().get("title").unwrap_or("").to_string();
        octx.add_chapter(
            chapter.id(),
            chapter.time_base(),
            chapter.start(),
            chapter.end(),
            title,
        )
        .map_err(|error| decrypt_failure(&format!("copying chapter {}", chapter.id()), error))?;
    }
    Ok(())
}

fn copy_packets(
    ictx: &mut ff::format::context::Input,
    octx: &mut ff::format::context::Output,
    mapping: &[Option<usize>],
    control: &CopyControl,
) -> Result<()> {
    let duration_seconds = ictx.duration() as f64 / f64::from(ff::ffi::AV_TIME_BASE);
    while let Some(mut packet) = next_packet(ictx, control)? {
        let Some(out_index) = mapping.get(packet.stream()).copied().flatten() else {
            continue;
        };
        let in_base = stream_time_base(ictx, packet.stream())?;
        if let Some(fraction) = packet_fraction(packet.pts(), in_base, duration_seconds) {
            control.set_fraction(fraction);
        }
        let out_base = stream_time_base_out(octx, out_index)?;
        packet.set_stream(out_index);
        packet.rescale_ts(in_base, out_base);
        packet
            .write_interleaved(octx)
            .map_err(|error| decrypt_failure("writing audio", error))?;
    }
    Ok(())
}

fn next_packet(
    ictx: &mut ff::format::context::Input,
    control: &CopyControl,
) -> Result<Option<ff::Packet>> {
    if control.stop_requested() {
        return Err(remote_acquisition_cancelled());
    }
    let mut packet = ff::Packet::empty();
    match packet.read(ictx) {
        Ok(()) => Ok(Some(packet)),
        Err(ff::Error::Eof) => Ok(None),
        Err(error) => Err(decrypt_failure("reading audio", error)),
    }
}

fn stream_time_base(ictx: &ff::format::context::Input, index: usize) -> Result<ff::Rational> {
    ictx.stream(index)
        .map(|stream| stream.time_base())
        .ok_or_else(|| decrypt_failure("reading audio", "a packet names a missing stream"))
}

fn stream_time_base_out(octx: &ff::format::context::Output, index: usize) -> Result<ff::Rational> {
    octx.stream(index)
        .map(|stream| stream.time_base())
        .ok_or_else(|| decrypt_failure("writing audio", "an output stream is missing"))
}

/// How far a packet's time is through the book, or `None` when either is
/// unknown.
fn packet_fraction(
    pts: Option<i64>,
    time_base: ff::Rational,
    duration_seconds: f64,
) -> Option<f32> {
    if duration_seconds <= 0.0 || time_base.denominator() == 0 {
        return None;
    }
    let seconds =
        pts? as f64 * f64::from(time_base.numerator()) / f64::from(time_base.denominator());
    Some((seconds / duration_seconds).clamp(0.0, 1.0) as f32)
}

#[cfg(test)]
#[path = "decrypt_tests.rs"]
mod tests;
