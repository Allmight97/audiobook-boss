//! Metadata passthrough helpers (chapters, cover art) for preserving source metadata.
//!
//! This module extracts metadata that should be copied from source files
//! without re-encoding or rewriting (chapters and original cover art).

use std::path::PathBuf;

use ffmpeg_next as ff;

use crate::errors::Result;
use crate::metadata::AudiobookMetadata;

/// Metadata-owned facts needed to extract chapters and cover art from source files.
#[derive(Debug, Clone)]
pub struct PassthroughSource {
    pub path: PathBuf,
    pub duration: Option<f64>,
    pub is_valid: bool,
}

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

impl PassthroughMetadata {
    pub fn into_option(self) -> Option<Self> {
        if self.chapters.is_empty() && self.cover_art.is_none() {
            None
        } else {
            Some(self)
        }
    }

    pub fn cover_art_only(mut self) -> Option<Self> {
        self.chapters.clear();
        self.into_option()
    }

    pub fn without_cover_art(mut self) -> Option<Self> {
        self.cover_art = None;
        self.into_option()
    }
}

pub(crate) fn merge_passthrough_cover_art(
    metadata: Option<AudiobookMetadata>,
    passthrough: Option<&PassthroughMetadata>,
) -> Option<AudiobookMetadata> {
    let passthrough_cover = passthrough
        .and_then(|value| value.cover_art.as_ref())
        .cloned();
    match metadata {
        Some(mut metadata) => {
            if metadata.cover_art.is_none() {
                metadata.cover_art = passthrough_cover;
            }
            Some(metadata)
        }
        None => passthrough_cover.map(|cover_art| {
            let mut metadata = AudiobookMetadata::new();
            metadata.cover_art = Some(cover_art);
            metadata
        }),
    }
}

/// Merge source and explicit cover, then make the winning bytes write-ready once.
///
/// JPEG already at or under the write target is left untouched. PNG and oversized
/// JPEG go through `optimize_cover_art` so both encoder paths mux MJPEG and
/// mp4ameta writes the same JPEG `covr`. Passthrough cover bytes are dropped after
/// they have been adopted so remux cannot re-select the raw original.
pub(crate) fn prepare_output_cover_art(
    metadata: Option<AudiobookMetadata>,
    passthrough: Option<PassthroughMetadata>,
) -> Result<(Option<AudiobookMetadata>, Option<PassthroughMetadata>)> {
    let mut metadata = merge_passthrough_cover_art(metadata, passthrough.as_ref());
    if let Some(cover) = metadata.as_mut().and_then(|value| value.cover_art.as_mut()) {
        if !cover.is_empty() {
            *cover = crate::metadata::prepare_cover_art_for_write(cover)?;
        }
    }
    let passthrough = if metadata
        .as_ref()
        .and_then(|value| value.cover_art.as_ref())
        .is_some_and(|bytes| !bytes.is_empty())
    {
        passthrough.and_then(PassthroughMetadata::without_cover_art)
    } else {
        passthrough
    };
    Ok((metadata, passthrough))
}

fn synthesize_chapters_from_files(files: &[PassthroughSource]) -> Vec<ChapterSpec> {
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
pub fn extract_passthrough_metadata(files: &[PassthroughSource]) -> PassthroughMetadata {
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

        // Update offset for next file using known duration (seconds).
        // Rounded like the synthetic-chapter math so both paths agree.
        if let Some(duration) = file.duration {
            cumulative_offset_ms += (duration * 1000.0).round() as i64;
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
    use crate::errors::AppError;

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
            Err(e) => {
                return Err(AppError::General(format!(
                    "Failed to add chapter {} ({}): {}",
                    idx, title, e
                )))
            }
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
    use super::{
        merge_passthrough_cover_art, prepare_output_cover_art, ChapterSpec, PassthroughMetadata,
    };
    use crate::metadata::{AudiobookMetadata, CoverArtPassthroughPolicy};

    #[test]
    fn into_option_returns_none_for_empty_passthrough() {
        assert!(PassthroughMetadata::default().into_option().is_none());
    }

    #[test]
    fn cover_art_only_drops_chapters_and_keeps_cover_art() {
        let passthrough = PassthroughMetadata {
            chapters: vec![ChapterSpec {
                title: Some("Chapter 1".to_string()),
                start_ms: 0,
                end_ms: 1_000,
            }],
            cover_art: Some(vec![1, 2, 3]),
        };

        let cover_only = passthrough.cover_art_only().expect("cover art passthrough");
        assert!(
            cover_only.chapters.is_empty(),
            "preview should not preserve chapters"
        );
        assert_eq!(cover_only.cover_art, Some(vec![1, 2, 3]));
    }

    #[test]
    fn cover_art_only_returns_none_when_passthrough_only_has_chapters() {
        let passthrough = PassthroughMetadata {
            chapters: vec![ChapterSpec {
                title: Some("Chapter 1".to_string()),
                start_ms: 0,
                end_ms: 1_000,
            }],
            cover_art: None,
        };

        assert!(passthrough.cover_art_only().is_none());
    }

    #[test]
    fn without_cover_art_preserves_chapters_and_drops_cover_art() {
        let passthrough = PassthroughMetadata {
            chapters: vec![ChapterSpec {
                title: Some("Chapter 1".to_string()),
                start_ms: 0,
                end_ms: 1_000,
            }],
            cover_art: Some(vec![1, 2, 3]),
        };

        let without_cover = passthrough
            .without_cover_art()
            .expect("chapter passthrough should remain");

        assert_eq!(without_cover.chapters.len(), 1);
        assert_eq!(without_cover.cover_art, None);
    }

    #[test]
    fn without_cover_art_returns_none_when_passthrough_only_has_cover_art() {
        let passthrough = PassthroughMetadata {
            chapters: Vec::new(),
            cover_art: Some(vec![1, 2, 3]),
        };

        assert!(passthrough.without_cover_art().is_none());
    }

    #[test]
    fn cover_art_passthrough_policy_drops_only_cover_art_after_explicit_clear() {
        let passthrough = PassthroughMetadata {
            chapters: vec![ChapterSpec {
                title: Some("Chapter 1".to_string()),
                start_ms: 0,
                end_ms: 1_000,
            }],
            cover_art: Some(vec![1, 2, 3]),
        };

        let effective = CoverArtPassthroughPolicy::SuppressAfterExplicitClear
            .apply_to_passthrough(Some(passthrough))
            .expect("chapter passthrough remains");

        assert_eq!(effective.chapters.len(), 1);
        assert_eq!(effective.cover_art, None);
    }

    #[test]
    fn merge_passthrough_cover_art_fills_only_missing_cover_art() {
        let passthrough = PassthroughMetadata {
            chapters: Vec::new(),
            cover_art: Some(vec![1, 2, 3]),
        };
        let metadata = AudiobookMetadata {
            title: Some("Known title".to_string()),
            cover_art: None,
            ..Default::default()
        };

        let merged = merge_passthrough_cover_art(Some(metadata), Some(&passthrough))
            .expect("metadata remains");

        assert_eq!(merged.title.as_deref(), Some("Known title"));
        assert_eq!(merged.cover_art, Some(vec![1, 2, 3]));
    }

    #[test]
    fn merge_passthrough_cover_art_keeps_explicit_cover_art() {
        let passthrough = PassthroughMetadata {
            chapters: Vec::new(),
            cover_art: Some(vec![1, 2, 3]),
        };
        let metadata = AudiobookMetadata {
            cover_art: Some(vec![9]),
            ..Default::default()
        };

        let merged = merge_passthrough_cover_art(Some(metadata), Some(&passthrough))
            .expect("metadata remains");

        assert_eq!(merged.cover_art, Some(vec![9]));
    }

    #[test]
    fn prepare_output_cover_art_drops_passthrough_cover_after_adopting_jpeg() {
        let jpeg = {
            use image::{DynamicImage, ImageBuffer, ImageFormat};
            use std::io::Cursor;
            let image =
                DynamicImage::ImageRgb8(ImageBuffer::from_pixel(64, 64, image::Rgb([1, 2, 3])));
            let mut source = Cursor::new(Vec::new());
            image
                .write_to(&mut source, ImageFormat::Jpeg)
                .expect("fixture jpeg");
            source.into_inner()
        };
        let passthrough = PassthroughMetadata {
            chapters: vec![ChapterSpec {
                title: Some("Chapter 1".to_string()),
                start_ms: 0,
                end_ms: 1_000,
            }],
            cover_art: Some(jpeg.clone()),
        };

        let (metadata, passthrough) = prepare_output_cover_art(None, Some(passthrough))
            .expect("write-ready cover should prepare");

        assert_eq!(
            metadata.expect("cover adopted onto metadata").cover_art,
            Some(jpeg)
        );
        let passthrough = passthrough.expect("chapters remain");
        assert_eq!(passthrough.chapters.len(), 1);
        assert_eq!(passthrough.cover_art, None);
    }
}
