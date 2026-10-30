use crate::metadata::field_schema::TagField;
use crate::metadata::{build_series_list, AudiobookMetadata};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MetadataOp {
    SetString { field: TagField, value: String },
    SetTrack { number: u32, total: Option<u32> },
    SetDisk { number: u32, total: Option<u32> },
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
    push_string_op(
        &mut ops,
        metadata.description.as_ref(),
        TagField::Description,
    );

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
            value: if field == TagField::Date {
                value.trim().to_string()
            } else {
                value.to_string()
            },
        });
    }
}

fn push_position_op(ops: &mut Vec<MetadataOp>, value: Option<(u32, Option<u32>)>, field: TagField) {
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

    if metadata.series.is_some() || metadata.subseries.is_some() {
        push_built_series_op(ops, series_value, TagField::Series);
    }

    if metadata.series_part.is_some() || metadata.subseries_part.is_some() {
        push_built_series_op(ops, series_part_value, TagField::SeriesPart);
    }
}

fn push_built_series_op(ops: &mut Vec<MetadataOp>, value: Option<String>, field: TagField) {
    match value {
        Some(value) if !value.trim().is_empty() => ops.push(MetadataOp::SetString { field, value }),
        _ => ops.push(MetadataOp::Clear(field)),
    }
}

pub(crate) fn log_write_plan(plan: &super::MetadataWritePlan, writer: &str, artifact: &str) {
    let sort = match &plan.album_sort {
        super::AlbumSortWriteAction::Preserve => "preserve",
        super::AlbumSortWriteAction::Set(_) => "set",
        super::AlbumSortWriteAction::Clear => "clear",
    };
    log_field_decisions(&plan.metadata, sort, writer, artifact);
}

pub(crate) fn log_field_decisions(
    metadata: &AudiobookMetadata,
    album_sort: &str,
    writer: &str,
    artifact: &str,
) {
    let ops = plan_metadata_field_ops(metadata);
    let decisions: Vec<_> = TagField::ALL
        .iter()
        .map(|field| {
            let action = if ops
                .iter()
                .any(|op| matches!(op, MetadataOp::Clear(f) if f == field))
            {
                "clear"
            } else if ops.iter().any(|op| match op {
                MetadataOp::SetString { field: f, .. } => f == field,
                MetadataOp::SetTrack { .. } => *field == TagField::Track,
                MetadataOp::SetDisk { .. } => *field == TagField::Disk,
                _ => false,
            }) {
                "set"
            } else {
                "preserve"
            };
            format!("{field:?}={action}")
        })
        .collect();
    log::info!(
        "metadata_plan artifact={artifact} writer={writer} fields={} album_sort={album_sort}",
        decisions.join(",")
    );
    if let Some(bytes) = metadata.cover_art.as_ref() {
        log::info!("metadata_cover artifact={artifact} writer={writer} source=explicit action={} bytes={} format={:?}",
            if bytes.is_empty() { "clear" } else { "set" }, bytes.len(), super::cover_art::detect_cover_art_format(bytes));
    }
}

#[cfg(test)]
#[path = "metadata_ops_tests.rs"]
mod tests;
