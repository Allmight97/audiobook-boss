//! Whether each listed title and source can be exported.

use std::collections::BTreeMap;

use crate::audio::{AudioFile, TitleStatus};

pub(super) fn publish(
    files: &mut [AudioFile],
    title_sources: &mut BTreeMap<String, Vec<AudioFile>>,
) {
    for source in title_sources.values_mut().flatten() {
        source.status = source.own_status();
    }
    for file in files {
        match title_sources.get(&file.input_id) {
            Some(sources) => {
                file.status = combined(sources, &mut file.error);
            }
            None => file.status = file.own_status(),
        }
    }
}

fn combined(sources: &[AudioFile], error: &mut Option<String>) -> TitleStatus {
    let failed = sources
        .iter()
        .find(|source| source.own_status() == TitleStatus::Failed);
    let removed = sources
        .iter()
        .find(|source| source.own_status() == TitleStatus::RemovedFinished);
    let Some(unusable) = failed.or(removed) else {
        return TitleStatus::Ready;
    };
    if unusable.error.is_some() {
        error.clone_from(&unusable.error);
    }
    unusable.own_status()
}

#[cfg(test)]
#[path = "title_status_tests.rs"]
mod tests;
