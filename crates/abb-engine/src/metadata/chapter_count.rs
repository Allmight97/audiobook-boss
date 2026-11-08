//! Which chapters ABB sends to the output when titles' sources are joined.

use std::path::Path;

use super::ChapterSpec;

/// One source of a title, in join order. `chapters` counts the chapters the
/// source brings: accepted ones, or those its container carries.
pub(crate) struct JoinedSource<'a> {
    pub path: &'a Path,
    pub duration: Option<f64>,
    pub chapters: usize,
}

pub(crate) struct JoinedChapters {
    /// One chapter per measured source, titled by the file name, when the join
    /// has several sources and none brings a chapter; empty otherwise.
    pub synthetic: Vec<ChapterSpec>,
    /// Sources the synthesis skipped for want of a length.
    pub warnings: Vec<String>,
    /// How many chapters the output carries: the synthetic ones, else the sum.
    pub count: usize,
}

pub(crate) fn join_chapters(sources: &[JoinedSource<'_>]) -> JoinedChapters {
    let total: usize = sources.iter().map(|source| source.chapters).sum();
    if sources.len() < 2 || total > 0 {
        return JoinedChapters {
            synthetic: Vec::new(),
            warnings: Vec::new(),
            count: total,
        };
    }

    let mut synthetic = Vec::new();
    let mut warnings = Vec::new();
    let mut offset_ms: i64 = 0;
    for source in sources {
        let Some(duration) = source.duration else {
            log::warn!(
                "Skipping synthetic chapter for {} due to missing duration",
                source.path.display()
            );
            warnings.push(no_length_warning(&source_name(source.path)));
            continue;
        };
        let duration_ms = (duration * 1000.0).round() as i64;
        synthetic.push(ChapterSpec {
            title: source
                .path
                .file_stem()
                .map(|stem| stem.to_string_lossy().into_owned()),
            start_ms: offset_ms,
            end_ms: offset_ms + duration_ms,
        });
        offset_ms += duration_ms;
    }
    JoinedChapters {
        count: synthetic.len(),
        synthetic,
        warnings,
    }
}

pub(super) fn source_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}

pub(super) fn no_length_warning(name: &str) -> String {
    format!("{name} has no known length, so the chapters after it may start too early.")
}

#[cfg(test)]
#[path = "chapter_count_tests.rs"]
mod tests;
