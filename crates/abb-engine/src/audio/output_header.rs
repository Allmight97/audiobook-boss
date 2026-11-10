use ffmpeg_next as ff;

use crate::errors::{AppError, Result};

/// FFmpeg's MP4-family muxers, which all take `movie_timescale`.
const MOVIE_TIMESCALE_MUXERS: [&str; 3] = ["mp4", "ipod", "mov"];

/// Writes an output header. For MP4 it pins the movie timescale to the audio
/// sample rate, which keeps edit lists sample-exact. Left unset, FFmpeg derives
/// it from every stream's time base (audio at 1/44100 plus a cover at 1/90000
/// gives 4,410,000), and the chapter track then truncates any chapter longer
/// than about 487 seconds. An option the muxer does not take is an error.
pub(crate) fn write_output_header(
    octx: &mut ff::format::context::Output,
    mut options: ff::Dictionary,
) -> Result<()> {
    if MOVIE_TIMESCALE_MUXERS.contains(&octx.format().name()) {
        options.set("movie_timescale", &audio_sample_rate(octx)?.to_string());
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
