use crate::errors::{AppError, Result};

pub fn write_output_header(
    output: abb_ffmpeg::Output,
    options: &abb_ffmpeg::Tags,
) -> Result<abb_ffmpeg::Muxer> {
    let mut options = options.clone();
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
        options.push("movie_timescale", &timescale, abb_ffmpeg::Step::WriteHeader)?;
    }
    let (muxer, unused) = output.write_header(&options)?;
    if unused.is_empty() {
        Ok(muxer)
    } else {
        Err(AppError::General(format!(
            "FFmpeg did not take the output options {}",
            unused.join(", ")
        )))
    }
}

const MOST_TICKS_BETWEEN_PACKETS: i128 = i32::MAX as i128 - 2;

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

#[cfg(test)]
#[path = "output_header_tests.rs"]
mod tests;
