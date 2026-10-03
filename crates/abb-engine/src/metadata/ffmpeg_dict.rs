//! FFmpeg metadata dictionary helpers for AudiobookMetadata.

use super::{
    compute_album_sort, split_series_list, AlbumSortWriteAction, AudiobookMetadata,
    MetadataWritePlan,
};
use crate::errors::Result;
use crate::metadata::field_schema::TagField;
use crate::metadata::metadata_sinks::{
    apply_metadata_field_ops_to_ffmpeg_dict, ExistingKeyRemoval,
};

use ffmpeg_next as ff;

pub fn metadata_to_ffmpeg_dict(metadata: &AudiobookMetadata) -> Result<ff::Dictionary<'_>> {
    let mut dict = ff::Dictionary::new();
    apply_metadata_field_ops_to_ffmpeg_dict(&mut dict, metadata)?;
    dict.set("media_type", "2");

    // Album sort is intentionally excluded from field ops and handled by write plans.
    if let Some(ref album_sort) = metadata.album_sort {
        if !album_sort.trim().is_empty() {
            dict.set("sort_album", album_sort);
        }
    }

    Ok(dict)
}

pub(crate) fn merge_metadata_with_plan<'a>(
    existing: ff::Dictionary<'a>,
    plan: &MetadataWritePlan,
) -> Result<ff::Dictionary<'a>> {
    let metadata = &plan.metadata;
    let removal = ExistingKeyRemoval::for_metadata(metadata);
    let clears_album_sort = matches!(
        plan.album_sort,
        AlbumSortWriteAction::Clear | AlbumSortWriteAction::Recompute
    );
    let mut merged = ff::Dictionary::new();
    for (key, value) in existing.iter() {
        if (clears_album_sort && key == "sort_album") || removal.removes(key) {
            continue;
        }
        merged.set(key, value);
    }

    let overrides = metadata_to_ffmpeg_dict(metadata)?;
    for (key, value) in overrides.iter() {
        merged.set(key, value);
    }

    match &plan.album_sort {
        AlbumSortWriteAction::Preserve => {}
        AlbumSortWriteAction::Set(value) => {
            if !value.trim().is_empty() {
                merged.set("sort_album", value);
            }
        }
        AlbumSortWriteAction::Clear => {}
        AlbumSortWriteAction::Recompute => {
            if let Some(album_sort) = compute_album_sort_from_dict(&merged) {
                merged.set("sort_album", &album_sort);
            }
        }
    }

    Ok(merged)
}

fn compute_album_sort_from_dict(dict: &ff::Dictionary<'_>) -> Option<String> {
    let series = first_tag(dict, TagField::Series);
    let series_part = first_tag(dict, TagField::SeriesPart);
    let title = dict.get("title").map(str::to_string);

    let (primary_series, _) = split_series_list(series.as_deref());
    let (primary_part, _) = split_series_list(series_part.as_deref());

    match (primary_series.as_deref(), title.as_deref()) {
        (Some(series), Some(title)) => compute_album_sort(series, primary_part.as_deref(), title),
        _ => None,
    }
}

fn first_tag(dict: &ff::Dictionary<'_>, field: TagField) -> Option<String> {
    field
        .read_keys()
        .iter()
        .find_map(|key| dict.get(key).map(str::to_string))
}

/// Sets global metadata on output format context
/// This applies metadata at the container level
pub fn set_container_metadata(
    octx: &mut ff::format::context::Output,
    metadata: &AudiobookMetadata,
) -> Result<()> {
    super::metadata_ops::log_field_decisions(
        metadata,
        if metadata
            .album_sort
            .as_ref()
            .is_some_and(|s| !s.trim().is_empty())
        {
            "set"
        } else {
            "preserve"
        },
        "ffmpeg_encode",
        "current_encoder",
    );
    let dict = metadata_to_ffmpeg_dict(metadata)?;
    octx.set_metadata(dict);

    log::debug!("Container metadata set via ffmpeg-next");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_dict() -> ff::Dictionary<'static> {
        let mut dict = ff::Dictionary::new();
        dict.set("title", "Existing Title");
        dict.set("series", "Existing Series");
        dict.set("series-part", "1");
        dict.set("sort_album", "Custom Sort");
        dict
    }

    #[test]
    fn comment_edits_replace_language_aliases_and_preserve_technical_comments() {
        for comment in [None, Some(""), Some("New comment")] {
            let mut source = ff::Dictionary::new();
            for (key, value) in [
                ("comment-eng", "English"),
                ("comment-comment-fra", "French"),
                ("comment-reader note-eng", "Reader note"),
                ("comment-iTunSMPB-eng", "gapless timing"),
                ("comment-iTunNORM-eng", "normalization"),
            ] {
                source.set(key, value);
            }
            let plan = MetadataWritePlan::from_metadata(AudiobookMetadata {
                comment: comment.map(str::to_owned),
                ..Default::default()
            });
            let merged =
                merge_metadata_with_plan(source, &plan).expect("apply comment edit to source tags");
            assert_eq!(merged.get("comment-iTunSMPB-eng"), Some("gapless timing"));
            assert_eq!(merged.get("comment-iTunNORM-eng"), Some("normalization"));
            for key in [
                "comment-eng",
                "comment-comment-fra",
                "comment-reader note-eng",
            ] {
                assert_eq!(merged.get(key).is_some(), comment.is_none());
            }
            assert_eq!(
                merged.get("comment"),
                comment.filter(|value| !value.is_empty())
            );
        }
    }

    #[test]
    fn merge_metadata_preserves_album_sort_without_explicit_intent() {
        let plan = MetadataWritePlan {
            metadata: AudiobookMetadata {
                genre: Some("Sci-Fi".to_string()),
                ..Default::default()
            },
            album_sort: AlbumSortWriteAction::Preserve,
        };

        let merged = merge_metadata_with_plan(base_dict(), &plan).expect("merge metadata");

        assert_eq!(merged.get("genre"), Some("Sci-Fi"));
        assert_eq!(merged.get("sort_album"), Some("Custom Sort"));
    }

    #[test]
    fn merge_metadata_sets_and_clears_album_sort_explicitly() {
        let set_plan = MetadataWritePlan::from_metadata(AudiobookMetadata {
            album_sort: Some("Requested Sort".to_string()),
            ..Default::default()
        });
        let set = merge_metadata_with_plan(base_dict(), &set_plan).expect("set album sort");
        assert_eq!(set.get("sort_album"), Some("Requested Sort"));

        let clear_plan = MetadataWritePlan::from_metadata(AudiobookMetadata {
            album_sort: Some(String::new()),
            ..Default::default()
        });
        let cleared = merge_metadata_with_plan(base_dict(), &clear_plan).expect("clear album sort");
        assert_eq!(cleared.get("sort_album"), None);
    }

    #[test]
    fn merge_metadata_recomputes_album_sort_only_when_requested() {
        let plan = MetadataWritePlan {
            metadata: AudiobookMetadata {
                series_part: Some("2".to_string()),
                ..Default::default()
            },
            album_sort: AlbumSortWriteAction::Recompute,
        };

        let merged = merge_metadata_with_plan(base_dict(), &plan).expect("recompute album sort");

        assert_eq!(
            merged.get("sort_album"),
            Some("Existing Series 02 - Existing Title")
        );
    }

    #[test]
    fn metadata_to_ffmpeg_dict_writes_literal_audiobook_fields() {
        let metadata = AudiobookMetadata {
            title: Some("Test Audiobook".to_string()),
            artist: Some("Test Author".to_string()),
            album: Some("Test Series".to_string()),
            composer: Some("Test Narrator".to_string()),
            genre: Some("Audiobook".to_string()),
            date: Some("2025".to_string()),
            description: Some("A test audiobook for metadata integration".to_string()),
            series: Some("Primary".to_string()),
            series_part: Some("7".to_string()),
            subseries: Some("Sub".to_string()),
            subseries_part: Some("2".to_string()),
            track: Some((4, Some(32))),
            disk: Some((1, Some(3))),
            ..Default::default()
        };

        let dict = metadata_to_ffmpeg_dict(&metadata).expect("metadata conversion should succeed");

        assert_eq!(dict.get("title"), Some("Test Audiobook"));
        assert_eq!(dict.get("artist"), Some("Test Author"));
        assert_eq!(dict.get("album_artist"), Some("Test Author"));
        assert_eq!(dict.get("album"), Some("Test Series"));
        assert_eq!(dict.get("composer"), Some("Test Narrator"));
        assert_eq!(dict.get("genre"), Some("Audiobook"));
        assert_eq!(dict.get("date"), Some("2025"));
        assert_eq!(dict.get("year"), Some("2025"));
        assert_eq!(
            dict.get("description"),
            Some("A test audiobook for metadata integration")
        );
        assert_eq!(dict.get("series"), Some("Primary; Sub"));
        assert_eq!(
            dict.get("----:com.apple.iTunes:SERIES"),
            Some("Primary; Sub")
        );
        assert_eq!(dict.get("series-part"), Some("7; 2"));
        assert_eq!(dict.get("----:com.apple.iTunes:SERIES-PART"), Some("7; 2"));
        assert_eq!(dict.get("track"), Some("4/32"));
        assert_eq!(dict.get("disc"), Some("1/3"));
        assert_eq!(dict.get("media_type"), Some("2"));
    }
}
