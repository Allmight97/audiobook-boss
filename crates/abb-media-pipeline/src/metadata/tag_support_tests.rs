use super::refuse_unholdable_edits;
use crate::metadata::container::Container;
use abb_metadata_core::{
    AlbumSortPatchOp, AudiobookMetadata, MetadataIntentPatch, MetadataWritePlan, PatchOp,
};

fn plan(patch: MetadataIntentPatch) -> MetadataWritePlan {
    MetadataWritePlan::from_patch(&patch, AudiobookMetadata::default()).expect("plan")
}

fn text(value: &str) -> Option<PatchOp<String>> {
    Some(PatchOp::Set(value.to_string()))
}

fn refusal(container: &str, patch: MetadataIntentPatch) -> Option<String> {
    refuse_unholdable_edits(&Container::from_format_name(container), &plan(patch))
        .err()
        .map(|error| error.to_string())
}

#[test]
fn raw_aac_refuses_any_edit() {
    for patch in [
        MetadataIntentPatch {
            title: text("T"),
            ..Default::default()
        },
        MetadataIntentPatch {
            album_sort: Some(AlbumSortPatchOp::Clear),
            ..Default::default()
        },
        MetadataIntentPatch {
            cover_art: Some(PatchOp::Clear),
            ..Default::default()
        },
    ] {
        assert!(refusal("aac", patch).expect("refused").contains(
            "Raw AAC (.aac) files can't hold tags. Export the title to M4B to keep them."
        ));
    }
}

#[test]
fn wav_refuses_only_the_fields_it_cannot_hold() {
    let held = MetadataIntentPatch {
        title: text("T"),
        artist: text("A"),
        album: text("B"),
        genre: text("G"),
        date: text("2020"),
        comment: text("C"),
        track: Some(PatchOp::Set((1, None))),
        ..Default::default()
    };
    assert_eq!(refusal("wav", held), None);

    let lost = MetadataIntentPatch {
        title: text("T"),
        description: text("D"),
        disk: Some(PatchOp::Set((1, None))),
        ..Default::default()
    };
    assert!(refusal("wav", lost).expect("refused").contains(
        "WAV files can't hold description or disc number. Export the title to M4B to keep them."
    ));

    let cover = MetadataIntentPatch {
        cover_art: Some(PatchOp::Clear),
        ..Default::default()
    };
    assert!(refusal("wav", cover)
        .expect("refused")
        .contains("WAV files can't hold cover. Export the title to M4B to keep it."));
}

#[test]
fn the_other_containers_hold_every_field() {
    let everything = MetadataIntentPatch {
        title: text("T"),
        composer: text("N"),
        description: text("D"),
        series: text("S"),
        series_part: text("1"),
        album_sort: Some(AlbumSortPatchOp::Clear),
        disk: Some(PatchOp::Set((1, None))),
        cover_art: Some(PatchOp::Clear),
        ..Default::default()
    };
    for container in ["mp4", "matroska", "mp3", "flac"] {
        assert_eq!(refusal(container, everything.clone()), None, "{container}");
    }
}
