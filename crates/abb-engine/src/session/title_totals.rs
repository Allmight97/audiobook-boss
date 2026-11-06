//! What each output title adds up to, from its sources.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::audio::AudioFile;
use crate::metadata::output_chapter_count;

#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TitleTotals {
    /// The chapters ABB sends to the output for this title.
    pub chapter_count: usize,
    pub duration_seconds: f64,
    pub size_bytes: f64,
}

#[derive(Debug, Clone, Default)]
pub(super) struct ListTotals {
    pub by_title: BTreeMap<String, TitleTotals>,
    pub total_size_bytes: f64,
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
    let mut totals = ListTotals::default();
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
        totals.total_size_bytes += valid.iter().map(|source| size_of(source)).sum::<f64>();
        totals.by_title.insert(
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
    totals
}

#[cfg(test)]
#[path = "title_totals_tests.rs"]
mod tests;
