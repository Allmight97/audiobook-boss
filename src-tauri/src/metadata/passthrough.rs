//! Metadata passthrough helpers (chapters, cover art) for preserving source metadata.
//!
//! This module extracts metadata that should be copied from source files
//! without re-encoding or rewriting (chapters and original cover art).

use ffmpeg_next as ff;

use crate::audio::AudioFile as PipelineAudioFile;
use crate::errors::Result;

/// Minimal chapter representation for passthrough (milliseconds time base).
#[derive(Debug, Clone)]
pub struct ChapterSpec {
    pub title: Option<String>,
    pub start_ms: i64,
    pub end_ms: i64,
}

/// Aggregated passthrough metadata collected from input files.
#[derive(Debug, Clone, Default)]
pub struct PassthroughMetadata {
    pub chapters: Vec<ChapterSpec>,
    pub cover_art: Option<Vec<u8>>,
}

fn synthesize_chapters_from_files(files: &[PipelineAudioFile]) -> Vec<ChapterSpec> {
    let mut chapters = Vec::new();
    let mut offset_ms: i64 = 0;

    for file in files.iter().filter(|f| f.is_valid) {
        let Some(duration) = file.duration else {
            log::warn!(
                "Skipping synthetic chapter for {} due to missing duration",
                file.path.display()
            );
            continue;
        };
        let duration_ms = (duration * 1000.0).round() as i64;
        let title = file
            .path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string());

        chapters.push(ChapterSpec {
            title,
            start_ms: offset_ms,
            end_ms: offset_ms + duration_ms,
        });

        offset_ms += duration_ms;
    }

    chapters
}

/// Extract chapters and original cover art from all valid input files.
/// Chapters are normalized to milliseconds and offset by cumulative durations
/// to preserve ordering for multi-file merges.
pub fn extract_passthrough_metadata(files: &[PipelineAudioFile]) -> PassthroughMetadata {
    // Ensure FFmpeg is initialized before probing chapters.
    let _ = ff::init();

    let mut passthrough = PassthroughMetadata::default();
    let mut cumulative_offset_ms: i64 = 0;

    for file in files.iter().filter(|f| f.is_valid) {
        match ff::format::input(&file.path) {
            Ok(ictx) => {
                // Collect chapters
                if ictx.nb_chapters() > 0 {
                    for chapter in ictx.chapters() {
                        let title = chapter.metadata().get("title").map(str::to_string);
                        let start_ms = rescale_to_ms(chapter.start(), chapter.time_base())
                            + cumulative_offset_ms;
                        let end_ms = rescale_to_ms(chapter.end(), chapter.time_base())
                            + cumulative_offset_ms;
                        passthrough.chapters.push(ChapterSpec {
                            title,
                            start_ms,
                            end_ms,
                        });
                    }
                }

                // Collect original cover art from the first file that provides it
                if passthrough.cover_art.is_none() {
                    passthrough.cover_art = extract_attached_pic(&ictx);
                }
            }
            Err(e) => {
                log::warn!(
                    "Failed to open input for metadata passthrough ({}): {}",
                    file.path.display(),
                    e
                );
            }
        }

        // Update offset for next file using known duration (seconds)
        if let Some(duration) = file.duration {
            cumulative_offset_ms += (duration * 1000.0) as i64;
        }
    }

    // If no chapters were found and we have multiple valid inputs, synthesize
    // one chapter per file using file order and filenames.
    let valid_file_count = files.iter().filter(|f| f.is_valid).count();
    if passthrough.chapters.is_empty() && valid_file_count > 1 {
        let synthetic = synthesize_chapters_from_files(files);
        if !synthetic.is_empty() {
            log::info!(
                "Synthesized {} chapters from input file order (no source chapters present)",
                synthetic.len()
            );
            passthrough.chapters = synthetic;
        } else {
            log::warn!("Could not synthesize chapters due to missing durations");
        }
    }

    passthrough
}

/// Adds chapters to an output context using millisecond time base (1/1000).
pub fn add_chapters_to_output(
    octx: &mut ff::format::context::Output,
    chapters: &[ChapterSpec],
) -> Result<usize> {
    let mut copied = 0;
    if chapters.is_empty() {
        return Ok(0);
    }

    let time_base = ff::Rational(1, 1000); // milliseconds
    for (idx, chapter) in chapters.iter().enumerate() {
        let title = chapter.title.clone().unwrap_or_default();
        match octx.add_chapter(
            idx as i64,
            time_base,
            chapter.start_ms,
            chapter.end_ms,
            &title,
        ) {
            Ok(_) => copied += 1,
            Err(e) => log::warn!("Failed to add chapter {} ({}): {}", idx, title, e),
        }
    }
    Ok(copied)
}

fn extract_attached_pic(ictx: &ff::format::context::Input) -> Option<Vec<u8>> {
    use ff::format::stream::Disposition;

    for stream in ictx.streams() {
        if stream.disposition().contains(Disposition::ATTACHED_PIC) {
            unsafe {
                let av_stream = stream.as_ptr();
                let pic = (*av_stream).attached_pic;
                if !pic.data.is_null() && pic.size > 0 {
                    let bytes = std::slice::from_raw_parts(pic.data, pic.size as usize);
                    return Some(bytes.to_vec());
                }
            }
        }
    }
    None
}

fn rescale_to_ms(value: i64, time_base: ff::Rational) -> i64 {
    use ffmpeg_next::Rational;
    use ffmpeg_next::Rescale;

    value.rescale(time_base, Rational(1, 1000))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn add_chapters_to_output_accepts_ms_chapters() {
        ffmpeg_next::init().expect("ffmpeg init");

        let temp_dir = TempDir::new().expect("temp dir");
        let output = temp_dir.path().join("chapters.m4b");
        let mut octx = ffmpeg_next::format::output(&output).expect("create output context");

        let chapters = vec![
            ChapterSpec {
                title: Some("One".into()),
                start_ms: 0,
                end_ms: 1000,
            },
            ChapterSpec {
                title: Some("Two".into()),
                start_ms: 1000,
                end_ms: 2000,
            },
        ];

        let added = add_chapters_to_output(&mut octx, &chapters).expect("add chapters");
        assert_eq!(added, 2);
    }

    #[test]
    fn synthesize_chapters_from_valid_files() {
        let files = vec![
            PipelineAudioFile {
                path: std::path::PathBuf::from("01_intro.mp3"),
                duration: Some(1.0),
                is_valid: true,
                ..PipelineAudioFile::new(std::path::PathBuf::new())
            },
            PipelineAudioFile {
                path: std::path::PathBuf::from("02_chapter.mp3"),
                duration: Some(2.5),
                is_valid: true,
                ..PipelineAudioFile::new(std::path::PathBuf::new())
            },
        ];

        let synthesized = synthesize_chapters_from_files(&files);
        assert_eq!(synthesized.len(), 2);
        assert_eq!(synthesized[0].title.as_deref(), Some("01_intro"));
        assert_eq!(synthesized[0].start_ms, 0);
        assert_eq!(synthesized[0].end_ms, 1000);
        assert_eq!(synthesized[1].title.as_deref(), Some("02_chapter"));
        assert_eq!(synthesized[1].start_ms, 1000);
        assert_eq!(synthesized[1].end_ms, 3500);
    }
}
