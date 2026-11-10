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
    let spans: Vec<ChapterSpan> = octx
        .chapters()
        .map(|chapter| ChapterSpan {
            start: chapter.start(),
            end: chapter.end(),
            time_base: (
                chapter.time_base().numerator(),
                chapter.time_base().denominator(),
            ),
        })
        .collect();
    Ok(movie_timescale_for(audio_sample_rate(octx)?, &spans))
}

struct ChapterSpan {
    start: i64,
    end: i64,
    time_base: (i32, i32),
}

fn movie_timescale_for(rate: i32, chapters: &[ChapterSpan]) -> i32 {
    (1..=rate)
        .filter(|divisor| rate % divisor == 0)
        .map(|divisor| rate / divisor)
        .find(|timescale| every_chapter_fits(chapters, *timescale))
        .unwrap_or(1)
}

fn every_chapter_fits(chapters: &[ChapterSpan], timescale: i32) -> bool {
    let mut previous_start = 0;
    chapters.iter().all(|chapter| {
        let start = movie_ticks(chapter.start, chapter.time_base, timescale);
        let end = movie_ticks(chapter.end, chapter.time_base, timescale);
        let gap = start - previous_start;
        previous_start = start;
        end - start < MOST_TICKS_BETWEEN_PACKETS && gap < MOST_TICKS_BETWEEN_PACKETS
    })
}

fn movie_ticks(value: i64, (numerator, denominator): (i32, i32), timescale: i32) -> i128 {
    i128::from(value) * i128::from(numerator) * i128::from(timescale)
        / i128::from(denominator.max(1))
}

/// `write_output_header` for an `abb_ffmpeg` output.
pub(crate) fn write_layer_output_header(output: abb_ffmpeg::Output) -> Result<abb_ffmpeg::Muxer> {
    let mut options = abb_ffmpeg::Tags::default();
    if super::is_mp4_family_format(&output.muxer_name()) {
        let rate = output
            .streams()
            .iter()
            .filter(|stream| stream.medium == abb_ffmpeg::Medium::Audio)
            .map(|stream| stream.sample_rate)
            .find(|rate| *rate > 0)
            .ok_or_else(|| AppError::General("an MP4 output has no audio sample rate".into()))?;
        let spans: Vec<ChapterSpan> = output
            .chapters()
            .iter()
            .map(|chapter| ChapterSpan {
                start: chapter.start,
                end: chapter.end,
                time_base: (chapter.time_base.numerator, chapter.time_base.denominator),
            })
            .collect();
        let timescale = movie_timescale_for(rate, &spans).to_string();
        options
            .push("movie_timescale", &timescale, abb_ffmpeg::Step::WriteHeader)
            .map_err(|error| AppError::General(error.to_string()))?;
    }
    let (muxer, unused) = output
        .write_header(&options)
        .map_err(|error| AppError::General(error.to_string()))?;
    if unused.is_empty() {
        Ok(muxer)
    } else {
        Err(AppError::General(format!(
            "FFmpeg did not take the output options {}",
            unused.join(", ")
        )))
    }
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
