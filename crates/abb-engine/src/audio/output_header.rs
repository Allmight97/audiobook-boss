use ffmpeg_next as ff;

use crate::errors::{AppError, Result};

/// Writes an output header. For MP4 it pins the movie timescale to the audio
/// sample rate, which keeps edit lists sample-exact. Left unset, FFmpeg derives
/// it from every stream's time base (audio at 1/44100 plus a cover at 1/90000
/// gives 4,410,000), and the chapter track then truncates any chapter longer
/// than about 487 seconds. The chapter track counts each chapter's duration in
/// 32 bits of movie ticks, and the muxer rejects a start more than 32 bits of
/// ticks after the previous chapter's start (the first, after zero), so a
/// longer chapter or a longer gap between starts takes a divisor of the sample
/// rate. An option the muxer does not take is an error.
pub(crate) fn write_output_header(
    octx: &mut ff::format::context::Output,
    mut options: ff::Dictionary,
) -> Result<()> {
    if super::is_mp4_family_format(octx.format().name()) {
        options.set("movie_timescale", &movie_timescale(octx)?.to_string());
    }
    let unused = octx.write_header_with(options)?;
    let names: Vec<&str> = unused.iter().map(|(name, _)| name).collect();
    if names.is_empty() {
        Ok(())
    } else {
        Err(AppError::General(format!(
            "FFmpeg did not take the output options {}",
            names.join(", ")
        )))
    }
}

/// FFmpeg rejects a delta of `i32::MAX` and rounds each start, so stay two
/// ticks below it.
const MOST_TICKS_BETWEEN_PACKETS: i128 = i32::MAX as i128 - 2;

fn movie_timescale(octx: &ff::format::context::Output) -> Result<i32> {
    let rate = audio_sample_rate(octx)?;
    Ok((1..=rate)
        .filter(|divisor| rate % divisor == 0)
        .map(|divisor| rate / divisor)
        .find(|timescale| every_chapter_fits(octx, *timescale))
        .unwrap_or(1))
}

fn every_chapter_fits(octx: &ff::format::context::Output, timescale: i32) -> bool {
    let mut previous_start = 0;
    octx.chapters().all(|chapter| {
        let start = movie_ticks(chapter.start(), chapter.time_base(), timescale);
        let end = movie_ticks(chapter.end(), chapter.time_base(), timescale);
        let gap = start - previous_start;
        previous_start = start;
        end - start < MOST_TICKS_BETWEEN_PACKETS && gap < MOST_TICKS_BETWEEN_PACKETS
    })
}

fn movie_ticks(value: i64, time_base: ff::Rational, timescale: i32) -> i128 {
    i128::from(value) * i128::from(time_base.numerator()) * i128::from(timescale)
        / i128::from(time_base.denominator().max(1))
}

fn audio_sample_rate(octx: &ff::format::context::Output) -> Result<i32> {
    octx.streams()
        .filter(|stream| stream.parameters().medium() == ff::media::Type::Audio)
        .map(|stream| {
            // SAFETY: copy the sample rate while the stream's parameters are borrowed.
            unsafe { (*stream.parameters().as_ptr()).sample_rate }
        })
        .find(|rate| *rate > 0)
        .ok_or_else(|| AppError::General("an MP4 output has no audio sample rate".into()))
}

#[cfg(test)]
#[path = "output_header_tests.rs"]
mod tests;
