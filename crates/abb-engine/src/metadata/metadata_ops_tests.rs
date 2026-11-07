use super::{plan_metadata_field_ops, MetadataOp};
use crate::metadata::AudiobookMetadata;
use abb_metadata_core::{PositionField, TagField, TextField};

#[test]
fn partial_subseries_name_writes_combined_series_without_part_clear() {
    let metadata = AudiobookMetadata {
        series: Some("Primary".to_string()),
        subseries: Some("Sub".to_string()),
        ..Default::default()
    };

    let ops = plan_metadata_field_ops(&metadata);

    assert!(ops.contains(&MetadataOp::SetText {
        field: TextField::Series,
        value: "Primary; Sub".to_string(),
    }));
    assert!(
        !ops.iter()
            .any(|op| matches!(op, MetadataOp::Clear(TagField::Text(TextField::SeriesPart)))),
        "a representable subseries name must not clear series-part"
    );
}

fn set_string(field: TextField, value: &str) -> MetadataOp {
    MetadataOp::SetText {
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
                MetadataOp::Clear(TagField::Text(TextField::Comment)),
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
                set_string(TextField::Series, "Wheel of Time"),
                set_string(TextField::SeriesPart, "3"),
                MetadataOp::SetPosition {
                    field: PositionField::Track,
                    number: 7,
                    total: Some(42),
                },
                MetadataOp::SetPosition {
                    field: PositionField::Disk,
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
                set_string(TextField::Series, "Primary; Sub"),
                set_string(TextField::SeriesPart, "1; 2"),
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
                set_string(TextField::Title, "Title"),
                MetadataOp::SetMediaTypeAudiobook,
            ],
        ),
    ];

    for (label, metadata, expected) in rows {
        assert_eq!(plan_metadata_field_ops(&metadata), expected, "{label}");
    }
}
