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
    pub operation_id: OperationId,
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
    directory: Option<String>,
    naming: OutputNamingConfig,
    answered: bool,
}

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

    pub(crate) fn acknowledge(&mut self, path: &Path, patch: &MetadataIntentPatch) {
        for link in self.links.values_mut().filter(|link| link.anchor == path) {
            link.acknowledged.merge(patch);
        }
    }

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

    pub(crate) fn offer(
        &mut self,
        edit: &OutputEdit,
        from: &Path,
        to: &Path,
        directory: Option<String>,
        naming: OutputNamingConfig,
    ) -> bool {
        let Some(link) = self.links.get(&edit.title_id) else {
            return false;
        };
        if !Arc::ptr_eq(&link.title, &edit.title) {
            return false;
        }
        let operation_id = link.operation_id.clone();
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
                    operation_id,
                    revision: edit.revision,
                    from: from.to_string_lossy().into_owned(),
                    to: to.to_string_lossy().into_owned(),
                },
                directory,
                naming,
                answered: false,
            },
        );
        true
    }

    pub(crate) fn accepts_reply(&self, edit: &OutputEdit) -> bool {
        self.links
            .get(&edit.title_id)
            .is_some_and(|link| Arc::ptr_eq(&link.title, &edit.title))
            && self
                .tickets
                .get(&edit.title_id)
                .is_none_or(|ticket| ticket.offer.revision <= edit.revision)
    }

    pub(crate) fn withdraw(&mut self, title_id: &str) {
        self.tickets.remove(title_id);
    }

    pub(crate) fn unlink(&mut self, title_id: &str) {
        self.links.remove(title_id);
        self.tickets.remove(title_id);
    }

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

    pub(crate) fn next_prompt(
        &self,
        directory: Option<&String>,
        naming: &OutputNamingConfig,
    ) -> Option<RestartOffer> {
        self.tickets
            .values()
            .filter(|ticket| {
                !ticket.answered
                    && ticket.directory.as_ref() == directory
                    && ticket.naming == *naming
            })
            .min_by(|a, b| a.offer.title_id.cmp(&b.offer.title_id))
            .map(|ticket| ticket.offer.clone())
    }

    pub(crate) fn acknowledge_prompt(&mut self, title_id: &str, revision: u64) {
        if let Some(ticket) = self
            .tickets
            .get_mut(title_id)
            .filter(|ticket| ticket.offer.revision == revision)
        {
            ticket.answered = true;
        }
    }

    pub(crate) fn consume_offer(&mut self, title_id: &str) {
        self.tickets.remove(title_id);
    }

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
#[path = "exports_tests.rs"]
mod tests;
