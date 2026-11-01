use super::{plan_metadata_field_ops, MetadataOp};
use crate::metadata::field_schema::TagField;
use crate::metadata::AudiobookMetadata;

#[test]
fn trim_guard_emits_clear_for_blank_string_fields() {
    let metadata = AudiobookMetadata {
        comment: Some("   ".to_string()),
        ..Default::default()
    };

    let ops = plan_metadata_field_ops(&metadata);

    assert!(ops.contains(&MetadataOp::Clear(TagField::Comment)));
    assert!(ops.contains(&MetadataOp::SetMediaTypeAudiobook));
}

#[test]
fn plans_series_track_and_disk_ops_together() {
    let metadata = AudiobookMetadata {
        series: Some("Wheel of Time".to_string()),
        series_part: Some("3".to_string()),
        track: Some((7, Some(42))),
        disk: Some((2, Some(5))),
        ..Default::default()
    };

    let ops = plan_metadata_field_ops(&metadata);

    assert!(ops.contains(&MetadataOp::SetString {
        field: TagField::Series,
        value: "Wheel of Time".to_string(),
    }));
    assert!(ops.contains(&MetadataOp::SetString {
        field: TagField::SeriesPart,
        value: "3".to_string(),
    }));
    assert!(ops.contains(&MetadataOp::SetTrack {
        number: 7,
        total: Some(42),
    }));
    assert!(ops.contains(&MetadataOp::SetDisk {
        number: 2,
        total: Some(5),
    }));
    assert!(ops
        .iter()
        .any(|op| matches!(op, MetadataOp::SetMediaTypeAudiobook)));
}

#[test]
fn subseries_intent_forces_series_write_ops() {
    let metadata = AudiobookMetadata {
        series: Some("Primary".to_string()),
        series_part: Some("1".to_string()),
        subseries: Some("Sub".to_string()),
        subseries_part: Some("2".to_string()),
        ..Default::default()
    };

    let ops = plan_metadata_field_ops(&metadata);

    assert!(ops.contains(&MetadataOp::SetString {
        field: TagField::Series,
        value: "Primary; Sub".to_string(),
    }));
    assert!(ops.contains(&MetadataOp::SetString {
        field: TagField::SeriesPart,
        value: "1; 2".to_string(),
    }));
}

#[test]
fn partial_subseries_name_writes_combined_series_without_part_clear() {
    let metadata = AudiobookMetadata {
        series: Some("Primary".to_string()),
        subseries: Some("Sub".to_string()),
        ..Default::default()
    };

    let ops = plan_metadata_field_ops(&metadata);

    assert!(ops.contains(&MetadataOp::SetString {
        field: TagField::Series,
        value: "Primary; Sub".to_string(),
    }));
    assert!(
        !ops.iter()
            .any(|op| matches!(op, MetadataOp::Clear(TagField::SeriesPart))),
        "a representable subseries name must not clear series-part"
    );
}

#[test]
fn album_sort_is_not_planned() {
    let metadata = AudiobookMetadata {
        album_sort: Some("Custom Sort".to_string()),
        title: Some("Title".to_string()),
        ..Default::default()
    };

    let ops = plan_metadata_field_ops(&metadata);

    assert_eq!(ops.len(), 2);
    assert!(ops.contains(&MetadataOp::SetString {
        field: TagField::Title,
        value: "Title".to_string(),
    }));
    assert!(ops.contains(&MetadataOp::SetMediaTypeAudiobook));
}
