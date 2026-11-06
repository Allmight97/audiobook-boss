//! What each output title adds up to, from its sources.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::audio::AudioFile;
use crate::metadata::output_chapter_count;
use crate::processing::{PreviewConfig, PREVIEW_SECONDS};

#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TitleTotals {
    /// The chapters ABB sends to the output for this title.
    pub chapter_count: usize,
    pub duration_seconds: f64,
    pub size_bytes: f64,
}

/// How long Preview Audio runs for the titles in the list, in whole seconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PreviewLength {
    pub shortest_seconds: u32,
    pub longest_seconds: u32,
}

impl PreviewLength {
    fn spanning(title_seconds: &[u32]) -> Self {
        let standard = PREVIEW_SECONDS as u32;
        Self {
            shortest_seconds: title_seconds.iter().copied().min().unwrap_or(standard),
            longest_seconds: title_seconds.iter().copied().max().unwrap_or(standard),
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct ListTotals {
    pub by_title: BTreeMap<String, TitleTotals>,
    pub total_size_bytes: f64,
    pub preview_length: PreviewLength,
}

fn accepted_chapter_count(file: &AudioFile) -> usize {
    file.chapter_plan
        .as_ref()
        .map_or(file.chapters.len(), |plan| plan.chapters.len())
}

pub(super) fn title_totals(
    files: &[AudioFile],
    title_sources: &BTreeMap<String, Vec<AudioFile>>,
) -> ListTotals {
    let mut by_title = BTreeMap::new();
    let mut total_size_bytes = 0.0;
    let mut preview_seconds = Vec::new();
    for file in files {
        let sources = title_sources
            .get(&file.input_id)
            .map_or(std::slice::from_ref(file), Vec::as_slice);
        let valid: Vec<&AudioFile> = sources.iter().filter(|source| source.is_valid).collect();
        let size_of = |source: &AudioFile| source.size.unwrap_or(0.0);
        let chapter_counts: Vec<usize> = valid
            .iter()
            .map(|source| accepted_chapter_count(source))
            .collect();
        total_size_bytes += valid.iter().map(|source| size_of(source)).sum::<f64>();
        if !valid.is_empty() {
            preview_seconds.push(PreviewConfig.title_seconds(valid.len()).round() as u32);
        }
        by_title.insert(
            file.input_id.clone(),
            TitleTotals {
                chapter_count: output_chapter_count(&chapter_counts),
                duration_seconds: sources
                    .iter()
                    .map(|source| source.duration.unwrap_or(0.0))
                    .sum(),
                size_bytes: sources.iter().map(size_of).sum(),
            },
        );
    }
    ListTotals {
        by_title,
        total_size_bytes,
        preview_length: PreviewLength::spanning(&preview_seconds),
    }
}

#[cfg(test)]
#[path = "title_totals_tests.rs"]
mod tests;
