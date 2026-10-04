//! The working set: output titles, their ordered sources, order, and selection.
//!
//! Every function here is a state transition with no I/O. A visible entry in
//! `files` anchors one output title's metadata; a grouped title keeps its
//! ordered sources in `title_sources`. Identity is the file's `input_id`, so
//! reorder and sort never change which title a choice belongs to.

use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::audio::{AudioFile, TitleAudioRequest};
use crate::errors::AppErrorEnvelope;
use crate::metadata::CueStatus;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum SortDirection {
    #[default]
    None,
    Ascending,
    Descending,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SelectionModifiers {
    pub multi: bool,
    pub range: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum MoveDirection {
    Up,
    Down,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum CueChoice {
    /// Accept a CUE whose timestamps need the hundredths interpretation.
    ConfirmHundredths,
    /// Drop the CUE and fall back to the file's embedded chapters.
    Ignore,
}

/// Why the last import added nothing. Hosts word these for the user.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum InputNotice {
    /// The order is locked while processing; nothing can be added.
    OrderLocked,
    #[serde(rename_all = "camelCase")]
    NoSupportedFiles {
        formats_text: String,
    },
    /// Every analyzed file was already in the list.
    DuplicatesOnly,
    DiscoveryFailed {
        error: AppErrorEnvelope,
    },
    AnalysisFailed {
        error: AppErrorEnvelope,
    },
}

/// The output titles as a host sees them. Selection travels separately
/// because it changes far more often than the titles do.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TitlesSnapshot {
    pub revision: u64,
    pub files: Vec<AudioFile>,
    /// Ordered sources for grouped titles, keyed by the title's identity.
    pub title_sources_by_identity: BTreeMap<String, Vec<AudioFile>>,
    /// Grouped titles whose sources disagreed on audio handling.
    pub audio_choice_required: Vec<String>,
    pub sort_direction: SortDirection,
    pub order_locked: bool,
    pub notice: Option<InputNotice>,
    pub order_differs_from_import: bool,
    /// Companion PDF names of downloaded titles, by input id.
    pub companions: BTreeMap<String, Vec<String>>,
    /// Advances whenever a Save writes a cover into a source, so an address
    /// for a source's cover names its current image.
    pub covers_revision: u64,
}

/// Which titles are selected, as positions in [`TitlesSnapshot::files`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SelectionSnapshot {
    pub revision: u64,
    pub selected_indices: Vec<usize>,
    pub selected_anchor: Option<usize>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct WorkingSet {
    files: Vec<AudioFile>,
    title_sources: BTreeMap<String, Vec<AudioFile>>,
    audio_choice_required: Vec<String>,
    selected_indices: Vec<usize>,
    selected_anchor: Option<usize>,
    sort_direction: SortDirection,
    order_locked: bool,
    notice: Option<InputNotice>,
    import_ordinal_by_path: HashMap<PathBuf, u64>,
    next_import_ordinal: u64,
    audio_requests: BTreeMap<String, TitleAudioRequest>,
    /// Counts changes to anything [`TitlesSnapshot`] carries.
    titles_changes: u64,
    /// Counts changes to audio requests.
    audio_changes: u64,
}

fn identity(file: &AudioFile) -> &str {
    &file.input_id
}

impl WorkingSet {
    pub(crate) fn titles(&self, revision: u64) -> TitlesSnapshot {
        TitlesSnapshot {
            revision,
            files: self.files.clone(),
            title_sources_by_identity: self.title_sources.clone(),
            audio_choice_required: self.audio_choice_required.clone(),
            sort_direction: self.sort_direction,
            order_locked: self.order_locked,
            notice: self.notice.clone(),
            order_differs_from_import: self.order_differs_from_import(),
            companions: BTreeMap::new(),
            covers_revision: 0,
        }
    }

    pub(crate) fn selection(&self, revision: u64) -> SelectionSnapshot {
        SelectionSnapshot {
            revision,
            selected_indices: self.selected_indices.clone(),
            selected_anchor: self.selected_anchor,
        }
    }

    /// Advances whenever the titles, their order, or their settings change.
    pub(crate) fn titles_changes(&self) -> u64 {
        self.titles_changes
    }

    fn touch(&mut self) {
        self.titles_changes += 1;
    }

    /// Advances whenever a title's audio request changes.
    pub(crate) fn audio_changes(&self) -> u64 {
        self.audio_changes
    }

    pub(crate) fn selected_indices(&self) -> &[usize] {
        &self.selected_indices
    }

    pub(crate) fn audio_choice_required(&self) -> &[String] {
        &self.audio_choice_required
    }

    pub(crate) fn files(&self) -> &[AudioFile] {
        &self.files
    }

    /// A downloaded source removed after export is no longer a valid input.
    /// Keep its row as history while excluding it from later standalone batches.
    pub(crate) fn sources_removed(&mut self, paths: &[PathBuf]) {
        for file in self
            .files
            .iter_mut()
            .chain(self.title_sources.values_mut().flatten())
        {
            if paths.contains(&file.path) {
                file.is_valid = false;
                file.error = Some(
                    "Downloaded source removed after export. Acquire it again to export it.".into(),
                );
            }
        }
        self.touch();
    }

    pub(crate) fn order_locked(&self) -> bool {
        self.order_locked
    }

    pub(crate) fn selected_files(&self) -> Vec<&AudioFile> {
        self.selected_indices
            .iter()
            .filter_map(|index| self.files.get(*index))
            .collect()
    }

    /// A title's ordered sources; an ungrouped title is its own single source.
    pub(crate) fn sources_for<'a>(&'a self, file: &'a AudioFile) -> &'a [AudioFile] {
        self.title_sources
            .get(identity(file))
            .map(Vec::as_slice)
            .unwrap_or(std::slice::from_ref(file))
    }

    /// Every source path in the set, including sources hidden inside groups.
    pub(crate) fn source_paths(&self) -> HashSet<PathBuf> {
        self.files
            .iter()
            .flat_map(|file| self.sources_for(file))
            .map(|source| source.path.clone())
            .collect()
    }

    /// Every source identity in the set, including sources hidden inside
    /// groups.
    pub(crate) fn source_ids(&self) -> HashSet<&str> {
        self.files
            .iter()
            .flat_map(|file| self.sources_for(file))
            .map(identity)
            .collect()
    }

    pub(crate) fn index_of(&self, title_id: &str) -> Option<usize> {
        self.files
            .iter()
            .position(|file| identity(file) == title_id)
    }

    fn order_differs_from_import(&self) -> bool {
        if self.files.len() <= 1 {
            return false;
        }
        let mut previous = None;
        for file in &self.files {
            let Some(ordinal) = self.import_ordinal_by_path.get(&file.path) else {
                return false;
            };
            if previous.is_some_and(|previous| ordinal < previous) {
                return true;
            }
            previous = Some(ordinal);
        }
        false
    }

    // ---- Import ----

    pub(crate) fn set_notice(&mut self, notice: InputNotice) {
        self.notice = Some(notice);
        self.touch();
    }

    /// Adds analyzed files as new titles. Files already present, visible or
    /// inside a group, are skipped; the first import into an empty set
    /// replaces it. Each new title takes `default_audio` as its request.
    pub(crate) fn append_analyzed(
        &mut self,
        analyzed: Vec<AudioFile>,
        default_audio: &TitleAudioRequest,
    ) {
        self.touch();
        if self.order_locked {
            self.notice = Some(InputNotice::OrderLocked);
            return;
        }
        let mut seen = self.source_paths();
        let replace = self.files.is_empty();
        let mut appended = Vec::new();
        for file in analyzed {
            if seen.insert(file.path.clone()) {
                appended.push(file);
            }
        }
        if !replace && appended.is_empty() {
            self.notice = Some(InputNotice::DuplicatesOnly);
            return;
        }

        if replace {
            self.import_ordinal_by_path.clear();
            self.next_import_ordinal = 0;
            self.sort_direction = SortDirection::None;
        }
        for file in &appended {
            if !self.import_ordinal_by_path.contains_key(&file.path) {
                self.import_ordinal_by_path
                    .insert(file.path.clone(), self.next_import_ordinal);
                self.next_import_ordinal += 1;
            }
            self.audio_requests
                .entry(identity(file).to_string())
                .or_insert_with(|| default_audio.clone());
        }
        self.files.extend(appended);
        if replace {
            let single_valid = self.files.len() == 1 && self.files[0].is_valid;
            self.selected_indices = if single_valid { vec![0] } else { Vec::new() };
            self.selected_anchor = single_valid.then_some(0);
        }
        self.notice = None;
    }

    // ---- Selection ----

    pub(crate) fn select_file(&mut self, index: usize, modifiers: SelectionModifiers) {
        if index >= self.files.len() {
            return;
        }
        if modifiers.range {
            if let Some(anchor) = self.selected_anchor {
                let (start, end) = (anchor.min(index), anchor.max(index));
                self.selected_indices = (start..=end).collect();
                self.selected_anchor = Some(index);
                return;
            }
        }
        if modifiers.multi {
            if let Some(position) = self.selected_indices.iter().position(|i| *i == index) {
                self.selected_indices.remove(position);
                self.selected_anchor = self.selected_indices.last().copied();
            } else {
                self.selected_indices.push(index);
                self.selected_indices.sort_unstable();
                self.selected_anchor = Some(index);
            }
            return;
        }
        self.selected_indices = vec![index];
        self.selected_anchor = Some(index);
    }

    pub(crate) fn select_all(&mut self) {
        if self.files.is_empty() {
            return;
        }
        self.selected_indices = (0..self.files.len()).collect();
        self.selected_anchor = Some(0);
    }

    pub(crate) fn clear_selection(&mut self) {
        self.selected_indices.clear();
        self.selected_anchor = None;
    }

    /// Keeps the anchor when it is still selected; otherwise moves it to the
    /// last selected index.
    fn reanchor(&mut self) {
        self.selected_indices.sort_unstable();
        let anchored = self
            .selected_anchor
            .is_some_and(|anchor| self.selected_indices.contains(&anchor));
        if !anchored {
            self.selected_anchor = self.selected_indices.last().copied();
        }
    }

    fn selected_ids(&self) -> (HashSet<String>, Option<String>) {
        let ids = self
            .selected_files()
            .into_iter()
            .map(|file| identity(file).to_string())
            .collect();
        let anchor = self
            .selected_anchor
            .and_then(|index| self.files.get(index))
            .map(|file| identity(file).to_string());
        (ids, anchor)
    }

    /// Re-derives selection indices from identities after the order changed.
    fn restore_selection(&mut self, ids: &HashSet<String>, anchor: Option<String>) {
        self.selected_indices = self
            .files
            .iter()
            .enumerate()
            .filter(|(_, file)| ids.contains(identity(file)))
            .map(|(index, _)| index)
            .collect();
        self.selected_anchor = match anchor {
            Some(anchor) => self
                .index_of(&anchor)
                .or_else(|| self.selected_indices.first().copied()),
            None => self.selected_indices.last().copied(),
        };
    }

    // ---- Removal ----

    /// Removes a title with all its sources. Returns the removed sources.
    pub(crate) fn remove_file(&mut self, index: usize) -> Vec<AudioFile> {
        if self.order_locked || index >= self.files.len() {
            return Vec::new();
        }
        self.touch();
        let removed = self.files.remove(index);
        let removed_id = identity(&removed).to_string();
        let sources = self
            .title_sources
            .remove(&removed_id)
            .unwrap_or_else(|| vec![removed]);
        for source in &sources {
            self.import_ordinal_by_path.remove(&source.path);
            self.title_sources.remove(identity(source));
            self.audio_requests.remove(identity(source));
        }
        self.audio_choice_required.retain(|id| id != &removed_id);
        self.selected_indices = self
            .selected_indices
            .iter()
            .filter(|selected| **selected != index)
            .map(|selected| {
                if *selected > index {
                    selected - 1
                } else {
                    *selected
                }
            })
            .collect();
        if self
            .selected_anchor
            .is_some_and(|anchor| anchor >= self.files.len())
        {
            self.selected_anchor = None;
        }
        self.reanchor();
        sources
    }

    /// Empties the set. Returns whether anything was cleared.
    pub(crate) fn clear_all(&mut self) -> bool {
        if self.order_locked || self.files.is_empty() {
            return false;
        }
        self.reset();
        true
    }

    /// Empties the set unconditionally.
    pub(crate) fn reset(&mut self) {
        *self = Self {
            titles_changes: self.titles_changes + 1,
            ..Self::default()
        };
    }

    // ---- Order ----

    /// Where the title with `title_id` is in the list now.
    pub(crate) fn title_index(&self, title_id: &str) -> Option<usize> {
        self.files.iter().position(|file| file.input_id == title_id)
    }

    pub(crate) fn move_file(&mut self, index: usize, direction: MoveDirection) {
        if self.order_locked || index >= self.files.len() {
            return;
        }
        let target = match direction {
            MoveDirection::Up if index > 0 => index - 1,
            MoveDirection::Down if index + 1 < self.files.len() => index + 1,
            _ => return,
        };
        self.touch();
        self.files.swap(index, target);
        let swap = |value: usize| {
            if value == index {
                target
            } else if value == target {
                index
            } else {
                value
            }
        };
        self.selected_indices = self.selected_indices.iter().map(|i| swap(*i)).collect();
        self.selected_anchor = self.selected_anchor.map(swap);
        self.reanchor();
        self.sort_direction = SortDirection::None;
    }

    pub(crate) fn reorder_files(&mut self, from: usize, to: usize) {
        let count = self.files.len();
        if self.order_locked || from == to || from >= count || to >= count {
            return;
        }
        self.touch();
        let moved = self.files.remove(from);
        self.files.insert(to, moved);
        let remap = |index: usize| {
            if index == from {
                to
            } else if from < to && index > from && index <= to {
                index - 1
            } else if from > to && index >= to && index < from {
                index + 1
            } else {
                index
            }
        };
        self.selected_indices = self.selected_indices.iter().map(|i| remap(*i)).collect();
        self.selected_indices.sort_unstable();
        self.selected_anchor = self.selected_anchor.map(remap);
        self.sort_direction = SortDirection::None;
    }

    /// Sorts titles by file name in natural numeric order, toggling direction.
    /// Selection follows the titles, not their positions.
    pub(crate) fn toggle_sort(&mut self) {
        if self.order_locked || self.files.len() <= 1 {
            return;
        }
        self.touch();
        let (ids, anchor) = self.selected_ids();
        let direction = if self.sort_direction == SortDirection::Ascending {
            SortDirection::Descending
        } else {
            SortDirection::Ascending
        };
        self.files.sort_by(|left, right| {
            let ordering = natural_cmp(&basename(&left.path), &basename(&right.path));
            if direction == SortDirection::Ascending {
                ordering
            } else {
                ordering.reverse()
            }
        });
        self.sort_direction = direction;
        self.restore_selection(&ids, anchor);
    }

    pub(crate) fn restore_import_order(&mut self) {
        if self.order_locked || self.files.len() <= 1 {
            return;
        }
        if self
            .files
            .iter()
            .any(|file| !self.import_ordinal_by_path.contains_key(&file.path))
        {
            return;
        }
        self.touch();
        let (ids, anchor) = self.selected_ids();
        let ordinals = &self.import_ordinal_by_path;
        self.files
            .sort_by_key(|file| ordinals.get(&file.path).copied().unwrap_or(0));
        self.sort_direction = SortDirection::None;
        self.restore_selection(&ids, anchor);
    }

    pub(crate) fn set_order_locked(&mut self, locked: bool) {
        if self.order_locked != locked {
            self.order_locked = locked;
            self.touch();
        }
    }

    // ---- Titles ----

    /// Merges the selected titles into one, anchored on the first selected
    /// title. Conflicting audio choices require an explicit choice afterward.
    /// Returns whether a group was formed.
    pub(crate) fn group_selected(&mut self) -> bool {
        if self.order_locked || self.selected_indices.len() < 2 {
            return false;
        }
        let mut indices = self.selected_indices.clone();
        indices.sort_unstable();
        let selected: Vec<AudioFile> = indices
            .iter()
            .filter_map(|index| self.files.get(*index).cloned())
            .collect();
        let Some(anchor) = selected.first() else {
            return false;
        };
        let key = identity(anchor).to_string();
        let selected_ids: HashSet<String> = selected
            .iter()
            .map(|file| identity(file).to_string())
            .collect();
        let sources: Vec<AudioFile> = selected
            .iter()
            .flat_map(|file| self.sources_for(file).to_vec())
            .collect();
        let first_choice = self.audio_requests.get(&key);
        let choices_differ = selected
            .iter()
            .any(|file| self.audio_requests.get(identity(file)) != first_choice);
        let choice_was_required = selected.iter().any(|file| {
            self.audio_choice_required
                .iter()
                .any(|id| id == identity(file))
        });

        self.touch();
        self.files
            .retain(|file| identity(file) == key || !selected_ids.contains(identity(file)));
        self.audio_choice_required
            .retain(|id| !selected_ids.contains(id));
        if choices_differ || choice_was_required {
            self.audio_choice_required.push(key.clone());
        }
        // Member titles stop being titles; their own groupings fold into this one.
        for id in &selected_ids {
            self.title_sources.remove(id);
        }
        let index = self.index_of(&key);
        self.title_sources.insert(key, sources);
        self.selected_indices = index.into_iter().collect();
        self.selected_anchor = index;
        self.sort_direction = SortDirection::None;
        true
    }

    /// Splits a grouped title back into its sources, in order, and selects them.
    pub(crate) fn ungroup(&mut self, title_id: &str) -> bool {
        if self.order_locked {
            return false;
        }
        let Some(index) = self.index_of(title_id) else {
            return false;
        };
        let Some(sources) = self.title_sources.get(title_id).cloned() else {
            return false;
        };
        if sources.len() < 2 {
            return false;
        }
        self.touch();
        for source in &sources {
            self.title_sources.remove(identity(source));
        }
        self.title_sources.remove(title_id);
        self.audio_choice_required.retain(|id| id != title_id);
        let count = sources.len();
        self.files.splice(index..=index, sources);
        self.selected_indices = (index..index + count).collect();
        self.selected_anchor = Some(index);
        true
    }

    pub(crate) fn reorder_sources(&mut self, title_id: &str, from: usize, to: usize) {
        if self.order_locked || self.index_of(title_id).is_none() {
            return;
        }
        let Some(sources) = self.title_sources.get_mut(title_id) else {
            return;
        };
        if from == to || from >= sources.len() || to >= sources.len() {
            return;
        }
        let moved = sources.remove(from);
        sources.insert(to, moved);
        self.touch();
    }

    /// Records the user's decision about one source's CUE sheet.
    pub(crate) fn choose_cue(&mut self, input_id: &str, choice: CueChoice) {
        if self.order_locked {
            return;
        }
        let apply = |file: &mut AudioFile| {
            if file.input_id != input_id {
                return;
            }
            let Some(cue) = file.cue_source.as_mut() else {
                return;
            };
            match choice {
                CueChoice::ConfirmHundredths if cue.status == CueStatus::NeedsConfirmation => {
                    cue.status = CueStatus::Ready;
                }
                CueChoice::Ignore if cue.status != CueStatus::EmbeddedPreferred => {
                    cue.status = CueStatus::Ignored;
                    if let Some(plan) = file.chapter_plan.as_mut() {
                        plan.from_cue = false;
                        plan.chapters = file.chapters.clone();
                    }
                }
                _ => {}
            }
        };
        self.files.iter_mut().for_each(apply);
        self.title_sources
            .values_mut()
            .flat_map(|sources| sources.iter_mut())
            .for_each(apply);
        self.touch();
    }

    /// Sets a title's audio request and settles a pending audio choice.
    pub(crate) fn set_audio_request(&mut self, title_id: &str, request: TitleAudioRequest) {
        if self.order_locked || self.index_of(title_id).is_none() {
            return;
        }
        self.audio_requests.insert(title_id.to_string(), request);
        self.audio_changes += 1;
        // Requests travel in the audio part; the titles part changes only
        // when a pending choice is settled.
        let required = self.audio_choice_required.len();
        self.audio_choice_required.retain(|id| id != title_id);
        if self.audio_choice_required.len() != required {
            self.touch();
        }
    }

    pub(crate) fn audio_request(&self, title_id: &str) -> Option<&TitleAudioRequest> {
        self.audio_requests.get(title_id)
    }
}

fn basename(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

/// Case-insensitive comparison that orders digit runs by numeric value, so
/// "Part 2" sorts before "Part 10".
fn natural_cmp(left: &str, right: &str) -> Ordering {
    let left: Vec<char> = left.to_lowercase().chars().collect();
    let right: Vec<char> = right.to_lowercase().chars().collect();
    let (mut i, mut j) = (0, 0);
    while i < left.len() && j < right.len() {
        if left[i].is_ascii_digit() && right[j].is_ascii_digit() {
            let digits = |chars: &[char], start: usize| {
                let end = chars[start..]
                    .iter()
                    .position(|c| !c.is_ascii_digit())
                    .map_or(chars.len(), |offset| start + offset);
                let run: String = chars[start..end].iter().collect();
                (run.trim_start_matches('0').to_string(), end)
            };
            let (left_run, left_end) = digits(&left, i);
            let (right_run, right_end) = digits(&right, j);
            let ordering = left_run
                .len()
                .cmp(&right_run.len())
                .then_with(|| left_run.cmp(&right_run));
            if ordering != Ordering::Equal {
                return ordering;
            }
            i = left_end;
            j = right_end;
        } else {
            let ordering = left[i].cmp(&right[j]);
            if ordering != Ordering::Equal {
                return ordering;
            }
            i += 1;
            j += 1;
        }
    }
    (left.len() - i).cmp(&(right.len() - j))
}

#[cfg(test)]
#[path = "working_set_tests.rs"]
mod tests;
