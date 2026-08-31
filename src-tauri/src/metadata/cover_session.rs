//! Session-scoped staged cover art for the Tauri Runtime Boundary.
//!
//! Display crosses IPC as a JPEG data URL (JSON string). Commit crosses as a
//! handle id. Bytes stay in this stash until save/process hydrates them into
//! `MetadataIntentPatch` for Metadata Outcome. Callers never round-trip
//! `Vec<u8>` through JSON `number[]`.
//!
//! The stash is process-memory: 256 entries, fail-closed when full, gone on
//! restart. Preview-from-URL must not insert, or lookup grids consume slots
//! that staged covers need at save.

use crate::errors::{AppError, Result};
use abb_metadata_core::{
    AlbumSortPatchOp, MetadataIntentPatch, MetadataIntentValidationResult, PatchOp,
};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

const COVER_STASH_MAX_ENTRIES: usize = 256;

static NEXT_COVER_HANDLE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CoverArtView {
    pub handle_id: Option<String>,
    pub data_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, Default)]
#[serde(rename = "MetadataIntentPatch")]
pub struct IpcMetadataIntentPatch {
    #[serde(default)]
    pub title: PatchOp<String>,
    #[serde(default)]
    pub artist: PatchOp<String>,
    #[serde(default)]
    pub album: PatchOp<String>,
    #[serde(default)]
    pub composer: PatchOp<String>,
    #[serde(default)]
    pub genre: PatchOp<String>,
    #[serde(default)]
    pub date: PatchOp<String>,
    #[serde(default)]
    pub description: PatchOp<String>,
    #[serde(default)]
    pub series: PatchOp<String>,
    #[serde(default)]
    pub series_part: PatchOp<String>,
    #[serde(default)]
    pub subseries: PatchOp<String>,
    #[serde(default)]
    pub subseries_part: PatchOp<String>,
    #[serde(default)]
    pub album_sort: AlbumSortPatchOp,
    #[serde(default)]
    pub cover_art: PatchOp<String>,
    #[serde(default)]
    pub comment: PatchOp<String>,
    #[serde(default)]
    pub track: PatchOp<(u32, Option<u32>)>,
    #[serde(default)]
    pub disk: PatchOp<(u32, Option<u32>)>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename = "MetadataIntentValidationResult", rename_all = "camelCase")]
pub struct IpcMetadataIntentValidationResult {
    pub is_valid: bool,
    pub metadata_patch: IpcMetadataIntentPatch,
    pub field_errors: Vec<abb_metadata_core::MetadataIntentFieldError>,
}

#[derive(Clone, Default)]
pub struct CoverStash {
    inner: Arc<Mutex<CoverStashInner>>,
}

#[derive(Default)]
struct CoverStashInner {
    bytes_by_id: HashMap<String, Vec<u8>>,
}

impl CoverStash {
    pub fn insert(&self, bytes: Vec<u8>) -> Result<String> {
        if bytes.is_empty() {
            return Err(AppError::InvalidInput("Cover art is empty".to_string()));
        }
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| AppError::General("Cover stash lock was poisoned".to_string()))?;
        if inner.bytes_by_id.len() >= COVER_STASH_MAX_ENTRIES {
            return Err(AppError::InvalidInput(format!(
                "Too many covers are staged ({COVER_STASH_MAX_ENTRIES}). Save or process current covers before staging more."
            )));
        }
        let id = format!(
            "cover-{}",
            NEXT_COVER_HANDLE.fetch_add(1, Ordering::Relaxed)
        );
        inner.bytes_by_id.insert(id.clone(), bytes);
        Ok(id)
    }

    pub fn get(&self, handle: &str) -> Result<Vec<u8>> {
        let inner = self
            .inner
            .lock()
            .map_err(|_| AppError::General("Cover stash lock was poisoned".to_string()))?;
        inner.bytes_by_id.get(handle).cloned().ok_or_else(|| {
            AppError::InvalidInput("Cover art is no longer staged. Load it again.".to_string())
        })
    }
}

pub fn jpeg_data_url(bytes: &[u8]) -> String {
    format!("data:image/jpeg;base64,{}", STANDARD.encode(bytes))
}

pub fn cover_art_view_for_display(bytes: &[u8]) -> CoverArtView {
    CoverArtView {
        handle_id: None,
        data_url: jpeg_data_url(bytes),
    }
}

pub fn stage_cover_art(stash: &CoverStash, bytes: Vec<u8>) -> Result<CoverArtView> {
    let data_url = jpeg_data_url(&bytes);
    let handle_id = stash.insert(bytes)?;
    Ok(CoverArtView {
        handle_id: Some(handle_id),
        data_url,
    })
}

impl IpcMetadataIntentPatch {
    pub fn into_core(self, stash: &CoverStash) -> Result<MetadataIntentPatch> {
        Ok(MetadataIntentPatch {
            title: self.title,
            artist: self.artist,
            album: self.album,
            composer: self.composer,
            genre: self.genre,
            date: self.date,
            description: self.description,
            series: self.series,
            series_part: self.series_part,
            subseries: self.subseries,
            subseries_part: self.subseries_part,
            album_sort: self.album_sort,
            cover_art: match self.cover_art {
                PatchOp::Set(handle) => PatchOp::Set(stash.get(&handle)?),
                PatchOp::Clear => PatchOp::Clear,
                PatchOp::Noop => PatchOp::Noop,
            },
            comment: self.comment,
            track: self.track,
            disk: self.disk,
        })
    }

    pub fn from_validated(
        result: MetadataIntentValidationResult,
        cover_art: PatchOp<String>,
    ) -> IpcMetadataIntentValidationResult {
        IpcMetadataIntentValidationResult {
            is_valid: result.is_valid,
            metadata_patch: Self::from_core_text(result.metadata_patch, cover_art),
            field_errors: result.field_errors,
        }
    }

    fn from_core_text(core: MetadataIntentPatch, cover_art: PatchOp<String>) -> Self {
        Self {
            title: core.title,
            artist: core.artist,
            album: core.album,
            composer: core.composer,
            genre: core.genre,
            date: core.date,
            description: core.description,
            series: core.series,
            series_part: core.series_part,
            subseries: core.subseries,
            subseries_part: core.subseries_part,
            album_sort: core.album_sort,
            cover_art,
            comment: core.comment,
            track: core.track,
            disk: core.disk,
        }
    }

    pub fn into_core_skipping_cover(self) -> MetadataIntentPatch {
        MetadataIntentPatch {
            title: self.title,
            artist: self.artist,
            album: self.album,
            composer: self.composer,
            genre: self.genre,
            date: self.date,
            description: self.description,
            series: self.series,
            series_part: self.series_part,
            subseries: self.subseries,
            subseries_part: self.subseries_part,
            album_sort: self.album_sort,
            cover_art: PatchOp::Noop,
            comment: self.comment,
            track: self.track,
            disk: self.disk,
        }
    }
}

pub fn hydrate_intent_map(
    map: Option<HashMap<String, IpcMetadataIntentPatch>>,
    stash: &CoverStash,
) -> Result<Option<HashMap<String, MetadataIntentPatch>>> {
    let Some(map) = map else {
        return Ok(None);
    };
    let mut hydrated = HashMap::with_capacity(map.len());
    for (path, patch) in map {
        hydrated.insert(path, patch.into_core(stash)?);
    }
    Ok(Some(hydrated))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jpeg_data_url_is_base64_not_a_number_array() {
        let url = jpeg_data_url(&[0xff, 0xd8, 0xff]);
        assert!(url.starts_with("data:image/jpeg;base64,"));
        assert!(!url.contains('['));
    }

    #[test]
    fn stash_round_trips_bytes_and_rejects_when_full() {
        let stash = CoverStash::default();
        let mut ids = Vec::new();
        for index in 0..COVER_STASH_MAX_ENTRIES {
            ids.push(stash.insert(vec![index as u8]).expect("insert"));
        }
        let error = stash
            .insert(vec![255])
            .expect_err("full stash must not silently evict");
        assert!(error.to_string().contains("Too many covers"));
        assert_eq!(stash.get(&ids[0]).expect("first still live"), vec![0]);
        assert_eq!(
            stash.get(ids.last().expect("id")).expect("last still live"),
            vec![(COVER_STASH_MAX_ENTRIES - 1) as u8]
        );
    }

    #[test]
    fn ipc_cover_set_serializes_as_a_string_not_a_byte_array() {
        let patch = IpcMetadataIntentPatch {
            cover_art: PatchOp::Set("cover-1".to_string()),
            ..Default::default()
        };
        let json = serde_json::to_value(&patch).expect("serialize");
        assert_eq!(json["cover_art"]["op"], "set");
        assert_eq!(json["cover_art"]["value"], "cover-1");
        assert!(json["cover_art"]["value"].as_array().is_none());
    }

    #[test]
    fn hydrate_set_loads_stash_and_skip_cover_leaves_noop() {
        let stash = CoverStash::default();
        let handle = stash.insert(vec![7, 7, 7]).expect("insert");
        let hydrated = IpcMetadataIntentPatch {
            cover_art: PatchOp::Set(handle),
            title: PatchOp::Set("A".to_string()),
            ..Default::default()
        }
        .into_core(&stash)
        .expect("hydrate");
        assert_eq!(hydrated.cover_art, PatchOp::Set(vec![7, 7, 7]));
        assert_eq!(hydrated.title, PatchOp::Set("A".to_string()));

        let skipped = IpcMetadataIntentPatch {
            cover_art: PatchOp::Set("unused".to_string()),
            title: PatchOp::Set("B".to_string()),
            ..Default::default()
        }
        .into_core_skipping_cover();
        assert_eq!(skipped.cover_art, PatchOp::Noop);
        assert_eq!(skipped.title, PatchOp::Set("B".to_string()));
    }

    #[test]
    fn hydrate_unknown_handle_is_explicit_invalid_input() {
        let stash = CoverStash::default();
        let error = IpcMetadataIntentPatch {
            cover_art: PatchOp::Set("cover-missing".to_string()),
            ..Default::default()
        }
        .into_core(&stash)
        .expect_err("unknown handle must not become empty bytes");
        assert!(error.to_string().contains("no longer staged"));
    }

    #[test]
    fn display_view_has_no_commit_handle() {
        let view = cover_art_view_for_display(&[0xff, 0xd8, 0xff]);
        assert!(view.handle_id.is_none());
        assert!(view.data_url.starts_with("data:image/jpeg;base64,"));
    }
}
