use std::collections::HashMap;
use std::path::PathBuf;

use super::SessionState;
use crate::session::working_set::InputNotice;
use abb_media_pipeline::audio::{AudioFile, SkippedImportPath};
use abb_media_pipeline::output_artifact::SupplementalProcessingAsset;

impl SessionState {
    pub(crate) fn import_local(
        &mut self,
        files: Vec<AudioFile>,
        companions: &HashMap<PathBuf, Vec<SupplementalProcessingAsset>>,
        skipped: &[SkippedImportPath],
    ) {
        let before = self.working_set.source_paths();
        let default_audio = self.audio.request();
        self.working_set.append_analyzed(files, &default_audio);
        for file in self.working_set.files() {
            for source in self.working_set.sources_for(file) {
                if !source.is_valid {
                    continue;
                }
                let Some(assets) = companions.get(&source.path) else {
                    continue;
                };
                if !before.contains(&source.path) || !self.companions.has(&source.input_id) {
                    self.companions.attach(&source.input_id, assets.clone());
                }
            }
        }
        if !skipped.is_empty() {
            self.working_set.set_notice(InputNotice::SkippedPaths {
                skipped: skipped.to_vec(),
            });
        }
    }
}
