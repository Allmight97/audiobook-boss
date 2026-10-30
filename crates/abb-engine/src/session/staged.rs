use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::processing::SupplementalProcessingAsset;
use crate::work_runtime::{ChildJobSnapshot, ChildJobStatus};

#[derive(Debug, Default)]
pub(crate) struct StagedSources {
    /// By acquisition job id.
    jobs: BTreeMap<String, StagedJob>,
}

pub(crate) const RETRY_DELAY: Duration = Duration::from_secs(30);

#[derive(Debug, Default)]
struct StagedJob {
    /// By session input id.
    titles: BTreeMap<String, StagedTitle>,
    retry_at: Option<Instant>,
}

#[derive(Debug)]
struct StagedTitle {
    path: PathBuf,
    assets: Vec<SupplementalProcessingAsset>,
    finished: bool,
}

impl StagedSources {
    pub(crate) fn register(
        &mut self,
        job_id: &str,
        input_id: &str,
        path: PathBuf,
        assets: Vec<SupplementalProcessingAsset>,
    ) {
        self.jobs
            .entry(job_id.to_string())
            .or_default()
            .titles
            .insert(
                input_id.to_string(),
                StagedTitle {
                    path,
                    assets,
                    finished: false,
                },
            );
    }

    /// Records a download nothing was imported from, so a sweep removes it
    /// and retries if removal fails.
    pub(crate) fn register_unimported(&mut self, job_id: &str, paths: Vec<PathBuf>) {
        let job = self.jobs.entry(job_id.to_string()).or_default();
        for (index, path) in paths.into_iter().enumerate() {
            job.titles.insert(
                format!("unimported:{index}"),
                StagedTitle {
                    path,
                    assets: Vec::new(),
                    finished: true,
                },
            );
        }
    }

    fn title(&self, input_id: &str) -> Option<&StagedTitle> {
        self.jobs.values().find_map(|job| job.titles.get(input_id))
    }

    pub(crate) fn assets_for<'a>(
        &self,
        input_ids: impl IntoIterator<Item = &'a str>,
    ) -> Option<HashMap<String, Vec<SupplementalProcessingAsset>>> {
        let assets: HashMap<_, _> = input_ids
            .into_iter()
            .filter_map(|input_id| {
                let title = self.title(input_id)?;
                (!title.assets.is_empty()).then(|| (input_id.to_string(), title.assets.clone()))
            })
            .collect();
        (!assets.is_empty()).then_some(assets)
    }

    pub(crate) fn companions(&self) -> BTreeMap<String, Vec<String>> {
        self.jobs
            .values()
            .flat_map(|job| &job.titles)
            .filter(|(_, title)| !title.assets.is_empty())
            .map(|(input_id, title)| {
                let names = title
                    .assets
                    .iter()
                    .map(|asset| asset.file_name.clone())
                    .collect();
                (input_id.clone(), names)
            })
            .collect()
    }

    pub(crate) fn finish_export(&mut self, children: &[ChildJobSnapshot]) -> bool {
        let published = children
            .iter()
            .filter(|child| {
                child.status == ChildJobStatus::Completed && child.supplemental_warning.is_none()
            })
            .flat_map(|child| child.source_input_ids.iter().map(String::as_str));
        self.finish(published)
    }

    fn finish<'a>(&mut self, input_ids: impl IntoIterator<Item = &'a str>) -> bool {
        let mut any = false;
        for input_id in input_ids {
            for job in self.jobs.values_mut() {
                if let Some(title) = job.titles.get_mut(input_id) {
                    any |= !title.finished;
                    title.finished = true;
                }
            }
        }
        any
    }

    pub(crate) fn finish_unlisted(&mut self, listed: &HashSet<&str>) -> bool {
        let mut any = false;
        for title in self
            .jobs
            .values_mut()
            .flat_map(|job| job.titles.iter_mut())
            .filter(|(input_id, _)| !listed.contains(input_id.as_str()))
            .map(|(_, title)| title)
        {
            any |= !title.finished;
            title.finished = true;
        }
        any
    }

    pub(crate) fn removable(&self, in_use: &HashSet<PathBuf>, now: Instant) -> Vec<String> {
        self.jobs
            .iter()
            .filter(|(_, job)| {
                job.retry_at.is_none_or(|at| now >= at)
                    && job.titles.values().all(|title| {
                        title.finished
                            && !in_use.contains(&title.path)
                            && !title
                                .assets
                                .iter()
                                .any(|asset| in_use.contains(&asset.path))
                    })
            })
            .map(|(job_id, _)| job_id.clone())
            .collect()
    }

    pub(crate) fn paths(&self, job_id: &str) -> Vec<PathBuf> {
        self.jobs
            .get(job_id)
            .into_iter()
            .flat_map(|job| job.titles.values())
            .flat_map(|title| {
                std::iter::once(title.path.clone())
                    .chain(title.assets.iter().map(|asset| asset.path.clone()))
            })
            .collect()
    }

    pub(crate) fn removed(&mut self, job_id: &str) {
        self.jobs.remove(job_id);
    }

    pub(crate) fn removal_failed(&mut self, job_id: &str, now: Instant) {
        if let Some(job) = self.jobs.get_mut(job_id) {
            job.retry_at = Some(now + RETRY_DELAY);
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.jobs.is_empty()
    }
}

#[cfg(test)]
#[path = "staged_tests.rs"]
pub(crate) mod tests;
