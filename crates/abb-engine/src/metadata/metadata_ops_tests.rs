use super::{plan_metadata_field_ops, MetadataOp};
use crate::metadata::field_schema::TagField;
use crate::metadata::AudiobookMetadata;

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

fn set_string(field: TagField, value: &str) -> MetadataOp {
    MetadataOp::SetString {
        field,
        value: value.to_string(),
    }
}

#[test]
fn field_ops_plan_each_metadata_shape_in_write_order() {
    let rows = [
        (
            "blank comment",
            AudiobookMetadata {
                comment: Some("   ".to_string()),
                ..Default::default()
            },
            vec![
                MetadataOp::Clear(TagField::Comment),
                MetadataOp::SetMediaTypeAudiobook,
            ],
        ),
        (
            "series track disk",
            AudiobookMetadata {
                series: Some("Wheel of Time".to_string()),
                series_part: Some("3".to_string()),
                track: Some((7, Some(42))),
                disk: Some((2, Some(5))),
                ..Default::default()
            },
            vec![
                set_string(TagField::Series, "Wheel of Time"),
                set_string(TagField::SeriesPart, "3"),
                MetadataOp::SetTrack {
                    number: 7,
                    total: Some(42),
                },
                MetadataOp::SetDisk {
                    number: 2,
                    total: Some(5),
                },
                MetadataOp::SetMediaTypeAudiobook,
            ],
        ),
        (
            "subseries",
            AudiobookMetadata {
                series: Some("Primary".to_string()),
                series_part: Some("1".to_string()),
                subseries: Some("Sub".to_string()),
                subseries_part: Some("2".to_string()),
                ..Default::default()
            },
            vec![
                set_string(TagField::Series, "Primary; Sub"),
                set_string(TagField::SeriesPart, "1; 2"),
                MetadataOp::SetMediaTypeAudiobook,
            ],
        ),
        (
            "album sort only rides the write plan",
            AudiobookMetadata {
                album_sort: Some("Custom Sort".to_string()),
                title: Some("Title".to_string()),
                ..Default::default()
            },
            vec![
                set_string(TagField::Title, "Title"),
                MetadataOp::SetMediaTypeAudiobook,
            ],
        ),
    ];

    for (label, metadata, expected) in rows {
        assert_eq!(plan_metadata_field_ops(&metadata), expected, "{label}");
    }
}
