//! Planned metadata field operations derived from `AudiobookMetadata`.

use crate::metadata::field_schema::TagField;
use crate::metadata::{build_series_list, AudiobookMetadata};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MetadataOp {
    SetString {
        field: TagField,
        value: String,
    },
    SetTrack {
        number: u32,
        total: Option<u32>,
    },
    SetDisk {
        number: u32,
        total: Option<u32>,
    },
    Clear(TagField),
    SetMediaTypeAudiobook,
}

pub fn plan_metadata_field_ops(metadata: &AudiobookMetadata) -> Vec<MetadataOp> {
    let mut ops = Vec::new();

    push_string_op(&mut ops, metadata.title.as_ref(), TagField::Title);
    push_string_op(&mut ops, metadata.artist.as_ref(), TagField::Artist);
    push_string_op(&mut ops, metadata.album.as_ref(), TagField::Album);
    push_string_op(&mut ops, metadata.composer.as_ref(), TagField::Composer);
    push_string_op(&mut ops, metadata.genre.as_ref(), TagField::Genre);
    push_string_op(&mut ops, metadata.date.as_ref(), TagField::Date);
    push_string_op(&mut ops, metadata.comment.as_ref(), TagField::Comment);
    push_string_op(&mut ops, metadata.description.as_ref(), TagField::Description);

    push_series_ops(metadata, &mut ops);
    push_position_op(&mut ops, metadata.track, TagField::Track);
    push_position_op(&mut ops, metadata.disk, TagField::Disk);

    if !ops.is_empty() {
        ops.push(MetadataOp::SetMediaTypeAudiobook);
    }

    ops
}

fn push_string_op(ops: &mut Vec<MetadataOp>, value: Option<&String>, field: TagField) {
    let Some(value) = value else {
        return;
    };

    if value.trim().is_empty() {
        ops.push(MetadataOp::Clear(field));
    } else {
        ops.push(MetadataOp::SetString {
            field,
            value: value.to_string(),
        });
    }
}

fn push_position_op(
    ops: &mut Vec<MetadataOp>,
    value: Option<(u32, Option<u32>)>,
    field: TagField,
) {
    let Some((number, total)) = value else {
        return;
    };

    if number == 0 {
        ops.push(MetadataOp::Clear(field));
        return;
    }

    let op = match field {
        TagField::Track => MetadataOp::SetTrack { number, total },
        TagField::Disk => MetadataOp::SetDisk { number, total },
        _ => return,
    };
    ops.push(op);
}

fn push_series_ops(metadata: &AudiobookMetadata, ops: &mut Vec<MetadataOp>) {
    let series_fields_present = metadata.series.is_some()
        || metadata.series_part.is_some()
        || metadata.subseries.is_some()
        || metadata.subseries_part.is_some();

    if !series_fields_present {
        return;
    }

    let (series_value, series_part_value) = build_series_list(
        metadata.series.as_deref(),
        metadata.series_part.as_deref(),
        metadata.subseries.as_deref(),
        metadata.subseries_part.as_deref(),
    );

    if metadata.series.is_some() {
        push_built_series_op(ops, series_value, TagField::Series);
    }

    if metadata.series_part.is_some() {
        push_built_series_op(ops, series_part_value, TagField::SeriesPart);
    }
}

fn push_built_series_op(ops: &mut Vec<MetadataOp>, value: Option<String>, field: TagField) {
    match value {
        Some(value) if !value.trim().is_empty() => ops.push(MetadataOp::SetString { field, value }),
        _ => ops.push(MetadataOp::Clear(field)),
    }
}

pub(crate) fn field_has_clear_intent(metadata: &AudiobookMetadata, field: TagField) -> bool {
    match field {
        TagField::Title => metadata.title.as_ref().is_some_and(|v| v.trim().is_empty()),
        TagField::Artist => metadata.artist.as_ref().is_some_and(|v| v.trim().is_empty()),
        TagField::Album => metadata.album.as_ref().is_some_and(|v| v.trim().is_empty()),
        TagField::Composer => metadata.composer.as_ref().is_some_and(|v| v.trim().is_empty()),
        TagField::Genre => metadata.genre.as_ref().is_some_and(|v| v.trim().is_empty()),
        TagField::Date => metadata.date.as_ref().is_some_and(|v| v.trim().is_empty()),
        TagField::Comment => metadata.comment.as_ref().is_some_and(|v| v.trim().is_empty()),
        TagField::Description => {
            metadata.description.as_ref().is_some_and(|v| v.trim().is_empty())
        }
        TagField::Series => metadata.series.as_ref().is_some_and(|v| v.trim().is_empty()),
        TagField::SeriesPart => metadata
            .series_part
            .as_ref()
            .is_some_and(|v| v.trim().is_empty()),
        TagField::Track => metadata.track.is_some_and(|(number, _)| number == 0),
        TagField::Disk => metadata.disk.is_some_and(|(number, _)| number == 0),
    }
}

#[cfg(test)]
mod tests {
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
        assert!(ops.iter().any(|op| matches!(op, MetadataOp::SetMediaTypeAudiobook)));
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
}