use std::path::PathBuf;

use super::chapter_count::{join_chapters, no_length_warning, source_name, JoinedSource};
use super::container::Container;
use crate::errors::Result;
use crate::metadata::{AudiobookMetadata, CoverArtPassthroughPolicy};

#[derive(Debug, Clone)]
pub struct PassthroughSource {
    pub path: PathBuf,
    pub duration: Option<f64>,
    pub is_valid: bool,
    pub chapters: Option<Vec<ChapterSpec>>,
}

pub use super::ChapterSpec;

#[derive(Debug, Clone, Default)]
pub struct PassthroughMetadata {
    pub chapters: Vec<ChapterSpec>,
    pub cover_art: Option<Vec<u8>>,
    /// Sources whose chapters or cover could not be read, or whose chapter
    /// offsets are off; Work Center shows them with the finished title.
    pub warnings: Vec<String>,
}

impl PassthroughMetadata {
    pub fn into_option(self) -> Option<Self> {
        if self.chapters.is_empty() && self.cover_art.is_none() {
            None
        } else {
            Some(self)
        }
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
/// they have been adopted so remux cannot re-select the raw original. A source
/// cover that cannot be prepared is dropped and the returned warning says so; a
/// chosen cover that cannot be prepared fails the title.
pub(crate) fn prepare_output_cover_art(
    metadata: Option<AudiobookMetadata>,
    passthrough: Option<PassthroughMetadata>,
    policy: CoverArtPassthroughPolicy,
) -> Result<(
    Option<AudiobookMetadata>,
    Option<PassthroughMetadata>,
    Option<String>,
)> {
    let mut metadata = merge_passthrough_cover_art(metadata, passthrough.as_ref());
    let Some(cover) = metadata
        .as_mut()
        .and_then(|value| value.cover_art.as_mut())
        .filter(|bytes| !bytes.is_empty())
    else {
        return Ok((metadata, passthrough, None));
    };
    let passthrough = passthrough.and_then(PassthroughMetadata::without_cover_art);
    match crate::metadata::prepare_cover_art_for_write(cover) {
        Ok(prepared) => *cover = prepared,
        Err(error) if policy != CoverArtPassthroughPolicy::Chosen => {
            log::warn!(
                "cover_art_plan decision=drop_source_cover reason=prepare_failed error={error}"
            );
            if let Some(value) = metadata.as_mut() {
                value.cover_art = None;
            }
            let warning =
                "The source file's cover could not be used, so this audiobook has no cover.";
            return Ok((metadata, passthrough, Some(warning.to_string())));
        }
        Err(error) => return Err(error),
    }
    Ok((metadata, passthrough, None))
}

/// Extract chapters and original cover art from all valid input files.
/// Chapters are normalized to milliseconds and offset by cumulative durations
/// to preserve ordering for multi-file merges.
pub fn extract_passthrough_metadata(files: &[PassthroughSource]) -> PassthroughMetadata {
    let mut passthrough = PassthroughMetadata::default();
    let mut cumulative_offset_ms: i64 = 0;

    let valid: Vec<&PassthroughSource> = files.iter().filter(|f| f.is_valid).collect();
    let mut chapter_counts = Vec::with_capacity(valid.len());
    let mut unmeasured = Vec::new();
    for (position, file) in valid.iter().enumerate() {
        let chapters_before = passthrough.chapters.len();
        match crate::metadata::open_input(&file.path) {
            Ok(ictx) => {
                if file.chapters.is_none() {
                    passthrough
                        .chapters
                        .extend(read_chapters(&ictx).into_iter().map(|chapter| ChapterSpec {
                            start_ms: chapter.start_ms + cumulative_offset_ms,
                            end_ms: chapter.end_ms + cumulative_offset_ms,
                            ..chapter
                        }));
                }

                if passthrough.cover_art.is_none() {
                    passthrough.cover_art = super::reader::extract_attached_pic(&ictx);
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

        if let Some(chapters) = &file.chapters {
            passthrough
                .chapters
                .extend(chapters.iter().map(|chapter| ChapterSpec {
                    title: chapter.title.clone(),
                    start_ms: chapter.start_ms + cumulative_offset_ms,
                    end_ms: chapter.end_ms + cumulative_offset_ms,
                }));
        }

        chapter_counts.push(passthrough.chapters.len() - chapters_before);

        // Rounded like the synthetic-chapter math so both paths agree.
        match file.duration {
            Some(duration) => cumulative_offset_ms += (duration * 1000.0).round() as i64,
            None => unmeasured.push((position, source_name(&file.path))),
        }
    }
    for (position, name) in unmeasured {
        if chapter_counts[position + 1..]
            .iter()
            .any(|&count| count > 0)
        {
            passthrough.warnings.push(no_length_warning(&name));
        }
    }

    let joined_sources: Vec<_> = valid
        .iter()
        .zip(&chapter_counts)
        .map(|(file, &chapters)| JoinedSource {
            path: &file.path,
            duration: file.duration,
            chapters,
        })
        .collect();
    let joined = join_chapters(&joined_sources);
    passthrough.warnings.extend(joined.warnings);
    if !joined.synthetic.is_empty() {
        log::info!(
            "Synthesized {} chapters from input file order (no source chapters present)",
            joined.synthetic.len()
        );
        passthrough.chapters = joined.synthetic;
    }

    passthrough
}

/// The chapters a file carries, titled from the `title` tag, with starts and
/// ends rounded to milliseconds. Import, passthrough and the final
/// verification read chapters through this, so they agree.
pub(crate) fn read_chapters(input: &abb_ffmpeg::Input) -> Vec<ChapterSpec> {
    chapter_specs(&input.chapters())
}

pub(crate) fn chapter_specs(chapters: &[abb_ffmpeg::Chapter]) -> Vec<ChapterSpec> {
    let milliseconds = abb_ffmpeg::Rational::new(1, 1000);
    chapters
        .iter()
        .map(|chapter| ChapterSpec {
            title: chapter.title().map(abb_ffmpeg::Text::display),
            start_ms: abb_ffmpeg::Rational::rescale(chapter.start, chapter.time_base, milliseconds),
            end_ms: abb_ffmpeg::Rational::rescale(chapter.end, chapter.time_base, milliseconds),
        })
        .collect()
}

/// A copy of `chapter` that carries only its title tag, with the title's
/// bytes as they are; a chapter without a title gets an empty one.
pub(crate) fn titled_chapter(
    chapter: &abb_ffmpeg::Chapter,
) -> std::result::Result<abb_ffmpeg::Chapter, abb_ffmpeg::Error> {
    let step = abb_ffmpeg::Step::AddChapter;
    let title = match chapter.title() {
        Some(title) => title.clone(),
        None => abb_ffmpeg::Text::new("", step)?,
    };
    let mut tags = abb_ffmpeg::Tags::default();
    tags.insert(abb_ffmpeg::Text::new("title", step)?, title);
    Ok(abb_ffmpeg::Chapter {
        tags,
        ..chapter.clone()
    })
}

/// Adds the accepted chapters to an output before its header, in
/// milliseconds, and returns how many it added.
pub(crate) fn add_chapters(
    output: &mut abb_ffmpeg::Output,
    chapters: &[ChapterSpec],
) -> Result<usize> {
    use crate::errors::AppError;

    let step = abb_ffmpeg::Step::AddChapter;
    for (idx, chapter) in chapters.iter().enumerate() {
        let title = chapter.title.clone().unwrap_or_default();
        let added = (|| {
            let mut tags = abb_ffmpeg::Tags::default();
            tags.push("title", &title, step)?;
            output.add_chapter(&abb_ffmpeg::Chapter {
                id: idx as i64,
                time_base: abb_ffmpeg::Rational::new(1, 1000),
                start: chapter.start_ms,
                end: chapter.end_ms,
                tags,
            })
        })();
        added.map_err(|e: abb_ffmpeg::Error| {
            AppError::General(format!("Failed to add chapter {idx} ({title}): {e}"))
        })?;
    }
    Ok(chapters.len())
}

pub fn verify_chapters(path: &std::path::Path, expected: &[ChapterSpec]) -> Result<()> {
    crate::diagnostics::stage("metadata_verify_chapters", path, || {
        verify_chapters_inner(path, expected)
    })
}

fn verify_chapters_inner(path: &std::path::Path, expected: &[ChapterSpec]) -> Result<()> {
    if expected.is_empty() {
        log::info!(
            "metadata_chapters artifact={} status=skipped reason=no_expected_chapters",
            crate::diagnostics::artifact_id(path)
        );
        return Ok(());
    }
    let input = crate::metadata::open_input(path)?;
    let actual = read_chapters(&input);
    let ends_stored = Container::of_input(&input).stores_chapter_ends();
    let mismatch = first_chapter_difference(&actual, expected, ends_stored);
    log::info!(
        "metadata_chapters artifact={} expected={} actual={} first_mismatch={:?}",
        crate::diagnostics::artifact_id(path),
        expected.len(),
        actual.len(),
        mismatch
    );
    if let Some((index, (a, e))) =
        mismatch.and_then(|index| Some((index, (actual.get(index)?, expected.get(index)?))))
    {
        log::info!("metadata_chapter_mismatch artifact={} index={} title_matches={} expected_start_ms={} actual_start_ms={} expected_end_ms={} actual_end_ms={}",
            crate::diagnostics::artifact_id(path), index, a.title.as_deref().unwrap_or("") == e.title.as_deref().unwrap_or(""),
            e.start_ms, a.start_ms, e.end_ms, a.end_ms);
    }
    if mismatch.is_some() {
        return Err(crate::errors::AppError::InvalidInput(
            "Final output did not preserve the accepted chapter names and positions.".into(),
        ));
    }
    Ok(())
}

/// The first chapter where `actual` and `expected` differ, a missing one
/// included. MP4 players end a chapter where the next starts (the last at the
/// audio end), so without `ends_stored` only titles and starts count.
pub(super) fn first_chapter_difference(
    actual: &[ChapterSpec],
    expected: &[ChapterSpec],
    ends_stored: bool,
) -> Option<usize> {
    actual
        .iter()
        .zip(expected)
        .position(|(a, e)| {
            a.title.as_deref().unwrap_or("") != e.title.as_deref().unwrap_or("")
                || a.start_ms != e.start_ms
                || ends_stored && a.end_ms != e.end_ms
        })
        .or_else(|| (actual.len() != expected.len()).then_some(actual.len().min(expected.len())))
}

#[cfg(test)]
#[path = "passthrough_tests.rs"]
mod tests;
