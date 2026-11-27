use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;

use abb_media_pipeline::output_artifact::SupplementalProcessingAsset;

#[derive(Debug, Default)]
pub(crate) struct Companions {
    by_input: BTreeMap<String, Vec<SupplementalProcessingAsset>>,
}

impl Companions {
    pub(crate) fn attach(&mut self, input_id: &str, assets: Vec<SupplementalProcessingAsset>) {
        if assets.is_empty() {
            return;
        }
        self.by_input.insert(input_id.to_string(), assets);
    }

    pub(crate) fn has(&self, input_id: &str) -> bool {
        self.by_input.contains_key(input_id)
    }

    pub(crate) fn assets_for<'a>(
        &self,
        input_ids: impl IntoIterator<Item = &'a str>,
    ) -> Option<HashMap<String, Vec<SupplementalProcessingAsset>>> {
        let assets: HashMap<_, _> = input_ids
            .into_iter()
            .filter_map(|input_id| {
                let assets = self.by_input.get(input_id)?;
                Some((input_id.to_string(), assets.clone()))
            })
            .collect();
        (!assets.is_empty()).then_some(assets)
    }

    pub(crate) fn names(&self) -> BTreeMap<String, Vec<String>> {
        self.by_input
            .iter()
            .map(|(input_id, assets)| {
                let names = assets.iter().map(|asset| asset.file_name.clone()).collect();
                (input_id.clone(), names)
            })
            .collect()
    }

    pub(crate) fn retain(&mut self, listed: &HashSet<&str>) {
        self.by_input
            .retain(|input_id, _| listed.contains(input_id.as_str()));
    }

    pub(crate) fn remove_paths(&mut self, paths: &[PathBuf]) {
        self.by_input.retain(|_, assets| {
            assets.retain(|asset| !paths.contains(&asset.path));
            !assets.is_empty()
        });
    }
}

#[cfg(test)]
#[path = "companions_tests.rs"]
mod tests;
