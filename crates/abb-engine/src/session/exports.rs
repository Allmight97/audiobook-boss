//! The exports the session submitted: which output each title's later Saves
//! update, and the restart offered when an edit would move an output that is
//! not published yet.
//!
//! A title links to its latest export while it stays listed. The edit an
//! output should carry is everything written to the title's source since the
//! export was accepted plus the edit still pending, so a Save that already
//! reached the source is not undone on the output.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Serialize;

use crate::metadata::MetadataIntentPatch;
use crate::output_artifact::OutputNamingConfig;
use crate::processing::TitleOutput;
use crate::work_runtime::OperationId;

/// An edit that would move a title's unpublished output. Hosts ask whether
/// to restart the title there (`RestartTitle`) or keep it (`KeepTitleLocation`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RestartOffer {
    pub title_id: String,
    /// Names this offer; a later Save replaces it with another.
    #[specta(type = specta_typescript::Number)]
    pub revision: u64,
    /// Where the output is being written.
    pub from: String,
    /// Where the edit names it.
    pub to: String,
}

#[derive(Debug, Clone)]
pub(crate) struct ExportLink {
    pub(crate) operation_id: OperationId,
    pub(crate) index: usize,
    pub(crate) title: Arc<TitleOutput>,
    anchor: PathBuf,
    /// Edits written to the anchor since the export was accepted.
    acknowledged: MetadataIntentPatch,
}

/// What one Save did to the outputs of titles in exports.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct OutputEdits {
    /// Outputs that take the edit, before or after publication.
    pub updated: usize,
    /// Of those, published outputs whose tags now name another folder; the
    /// file is not moved.
    pub elsewhere: usize,
    /// Unpublished outputs the edit would move; see `restart_offers`.
    pub restart_offered: usize,
    pub failed: usize,
}

/// One Save's edit for one linked output.
#[derive(Debug, Clone)]
pub(crate) struct OutputEdit {
    pub(crate) title_id: String,
    pub(crate) title: Arc<TitleOutput>,
    pub(crate) revision: u64,
    pub(crate) intent: MetadataIntentPatch,
}

#[derive(Debug, Clone)]
struct Ticket {
    offer: RestartOffer,
    /// The output folder and naming the offer was computed under.
    directory: Option<String>,
    naming: OutputNamingConfig,
}

/// Why a restart was not started.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RestartStale {
    /// No offer with that revision, or the output settings changed since.
    Stale,
}

#[derive(Debug, Default)]
pub(crate) struct Exports {
    links: HashMap<String, ExportLink>,
    tickets: HashMap<String, Ticket>,
    /// Locations the user chose not to restart titles at.
    declined: HashMap<String, PathBuf>,
    edits: u64,
}

impl Exports {
    /// Links each of `titles` (title id, anchor path) to its output in an
    /// accepted export, replacing an earlier export of the same title.
    pub(crate) fn link(
        &mut self,
        operation_id: &OperationId,
        titles: impl IntoIterator<Item = (String, PathBuf)>,
        outputs: &[Arc<TitleOutput>],
    ) {
        for (index, ((title_id, anchor), title)) in titles.into_iter().zip(outputs).enumerate() {
            self.tickets.remove(&title_id);
            self.declined.remove(&title_id);
            self.links.insert(
                title_id,
                ExportLink {
                    operation_id: operation_id.clone(),
                    index,
                    title: Arc::clone(title),
                    anchor,
                    acknowledged: MetadataIntentPatch::default(),
                },
            );
        }
    }

    /// Records that `patch` was written to the file at `path`.
    pub(crate) fn acknowledge(&mut self, path: &Path, patch: &MetadataIntentPatch) {
        for link in self.links.values_mut().filter(|link| link.anchor == path) {
            link.acknowledged.merge(patch);
        }
    }

    /// Forgets titles that left the list.
    pub(crate) fn retain_listed(&mut self, listed: &HashSet<&str>) {
        self.links
            .retain(|title_id, _| listed.contains(title_id.as_str()));
        self.tickets
            .retain(|title_id, _| listed.contains(title_id.as_str()));
        self.declined
            .retain(|title_id, _| listed.contains(title_id.as_str()));
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.links.is_empty()
    }

    /// The edit each linked output should carry now, given the edit still
    /// pending on a file.
    pub(crate) fn edits(
        &mut self,
        pending: impl Fn(&Path) -> Option<MetadataIntentPatch>,
    ) -> Vec<OutputEdit> {
        self.edits += 1;
        let revision = self.edits;
        self.links
            .iter()
            .map(|(title_id, link)| {
                let mut intent = link.acknowledged.clone();
                if let Some(pending) = pending(&link.anchor) {
                    intent.merge(&pending);
                }
                OutputEdit {
                    title_id: title_id.clone(),
                    title: Arc::clone(&link.title),
                    revision,
                    intent,
                }
            })
            .collect()
    }

    /// Offers to restart a title whose edit would move its output, unless
    /// the user already kept it where it is.
    pub(crate) fn offer(
        &mut self,
        edit: &OutputEdit,
        from: &Path,
        to: &Path,
        directory: Option<String>,
        naming: OutputNamingConfig,
    ) -> bool {
        // A reply from an older Save that arrived late changes nothing.
        let older = self
            .tickets
            .get(&edit.title_id)
            .is_some_and(|ticket| ticket.offer.revision > edit.revision);
        if older {
            return false;
        }
        if self
            .declined
            .get(&edit.title_id)
            .is_some_and(|kept| kept == to)
        {
            return false;
        }
        self.tickets.insert(
            edit.title_id.clone(),
            Ticket {
                offer: RestartOffer {
                    title_id: edit.title_id.clone(),
                    revision: edit.revision,
                    from: from.to_string_lossy().into_owned(),
                    to: to.to_string_lossy().into_owned(),
                },
                directory,
                naming,
            },
        );
        true
    }

    /// The edit no longer moves the output.
    pub(crate) fn withdraw(&mut self, title_id: &str) {
        self.tickets.remove(title_id);
    }

    /// Forgets a link whose title ended without an output.
    pub(crate) fn unlink(&mut self, title_id: &str) {
        self.links.remove(title_id);
        self.tickets.remove(title_id);
    }

    /// The export to restart for `title_id`'s offer, if it is still the one
    /// the user saw and the output settings are the ones it was made under.
    /// The offer stays until `consume_offer`.
    pub(crate) fn offered(
        &self,
        title_id: &str,
        revision: u64,
        directory: Option<&String>,
        naming: &OutputNamingConfig,
    ) -> Result<ExportLink, RestartStale> {
        let current = self.tickets.get(title_id).is_some_and(|ticket| {
            ticket.offer.revision == revision
                && ticket.directory.as_ref() == directory
                && ticket.naming == *naming
        });
        let link = self.links.get(title_id).filter(|_| current).cloned();
        link.ok_or(RestartStale::Stale)
    }

    pub(crate) fn consume_offer(&mut self, title_id: &str) {
        self.tickets.remove(title_id);
    }

    /// The user keeps the output where it is.
    pub(crate) fn decline(&mut self, title_id: &str, revision: u64) -> bool {
        let Some(ticket) = self
            .tickets
            .get(title_id)
            .filter(|ticket| ticket.offer.revision == revision)
        else {
            return false;
        };
        let to = PathBuf::from(&ticket.offer.to);
        self.tickets.remove(title_id);
        self.declined.insert(title_id.to_string(), to);
        true
    }

    pub(crate) fn offers(&self) -> Vec<RestartOffer> {
        let mut offers: Vec<RestartOffer> = self
            .tickets
            .values()
            .map(|ticket| ticket.offer.clone())
            .collect();
        offers.sort_by(|a, b| a.title_id.cmp(&b.title_id));
        offers
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::PatchOp;
    use crate::processing::title_output::TitleOutputPlan;

    fn output() -> Arc<TitleOutput> {
        TitleOutput::with_writer(
            TitleOutputPlan {
                anchor: PathBuf::from("/books/alpha.m4b"),
                sources: Vec::new(),
                base: None,
                accepted: None,
                output_dir: PathBuf::from("/library"),
                naming: OutputNamingConfig::default(),
                extension: "m4b".to_string(),
                requested: PathBuf::from("/library/alpha.m4b"),
            },
            Box::new(|_, _| Ok(())),
        )
        .expect("output")
    }

    fn set(field: &str, value: &str) -> MetadataIntentPatch {
        let op = Some(PatchOp::Set(value.to_string()));
        match field {
            "genre" => MetadataIntentPatch {
                genre: op,
                ..MetadataIntentPatch::default()
            },
            _ => MetadataIntentPatch {
                composer: op,
                ..MetadataIntentPatch::default()
            },
        }
    }

    fn linked() -> Exports {
        let mut exports = Exports::default();
        exports.link(
            &OperationId("operation-1".to_string()),
            [("alpha".to_string(), PathBuf::from("/books/alpha.m4b"))],
            &[output()],
        );
        exports
    }

    #[test]
    fn an_edit_already_written_to_the_source_stays_in_the_output_edit() {
        let mut exports = linked();
        exports.acknowledge(Path::new("/books/alpha.m4b"), &set("genre", "Mystery"));

        // Only the narrator is pending now; the genre went to the source.
        let edits = exports.edits(|_| Some(set("narrator", "Reader")));

        let intent = &edits[0].intent;
        assert_eq!(intent.genre, Some(PatchOp::Set("Mystery".to_string())));
        assert_eq!(intent.composer, Some(PatchOp::Set("Reader".to_string())));
    }

    #[test]
    fn an_offer_is_taken_only_as_shown_and_a_kept_location_is_not_offered_again() {
        let mut exports = linked();
        let naming = OutputNamingConfig::default();
        let directory = Some("/library".to_string());
        let edit = exports.edits(|_| None).remove(0);
        let (from, to) = (Path::new("/library/a.m4b"), Path::new("/library/b.m4b"));
        assert!(exports.offer(&edit, from, to, directory.clone(), naming.clone()));

        assert!(exports
            .offered("alpha", edit.revision + 1, directory.as_ref(), &naming)
            .is_err());
        assert!(exports
            .offered(
                "alpha",
                edit.revision,
                Some(&"/elsewhere".to_string()),
                &naming
            )
            .is_err());

        assert!(exports
            .offered("alpha", edit.revision, directory.as_ref(), &naming)
            .is_ok());
        assert!(exports.decline("alpha", edit.revision));
        assert!(exports.offers().is_empty());
        let later = exports.edits(|_| None).remove(0);
        assert!(!exports.offer(&later, from, to, directory, naming));
    }
}
