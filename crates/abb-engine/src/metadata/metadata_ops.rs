use crate::metadata::{build_series_list, AudiobookMetadata};
use abb_metadata_core::{PositionField, TagField, TextField};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MetadataOp {
    SetText {
        field: TextField,
        value: String,
    },
    SetPosition {
        field: PositionField,
        number: u32,
        total: Option<u32>,
    },
    Clear(TagField),
    SetMediaTypeAudiobook,
}

/// The text fields whose tag value is the field itself; the series fields
/// combine the primary and sub-series (`push_series_ops`).
const PLAIN_TEXT_FIELDS: [TextField; 8] = [
    TextField::Title,
    TextField::Artist,
    TextField::Album,
    TextField::Composer,
    TextField::Genre,
    TextField::Date,
    TextField::Comment,
    TextField::Description,
];

pub fn plan_metadata_field_ops(metadata: &AudiobookMetadata) -> Vec<MetadataOp> {
    let mut ops = Vec::new();

    for field in PLAIN_TEXT_FIELDS {
        push_text_op(&mut ops, metadata.text(field), field);
    }

    push_series_ops(metadata, &mut ops);
    for field in PositionField::ALL {
        push_position_op(&mut ops, metadata.position(field), field);
    }

    if !ops.is_empty() {
        ops.push(MetadataOp::SetMediaTypeAudiobook);
    }

    ops
}

fn push_text_op(ops: &mut Vec<MetadataOp>, value: Option<&String>, field: TextField) {
    let Some(value) = value else {
        return;
    };

    if value.trim().is_empty() {
        ops.push(MetadataOp::Clear(TagField::Text(field)));
    } else {
        ops.push(MetadataOp::SetText {
            field,
            value: if field == TextField::Date {
                value.trim().to_string()
            } else {
                value.to_string()
            },
        });
    }
}

fn push_position_op(
    ops: &mut Vec<MetadataOp>,
    value: Option<(u32, Option<u32>)>,
    field: PositionField,
) {
    let Some((number, total)) = value else {
        return;
    };

    if number == abb_metadata_core::POSITION_CLEAR_NUMBER {
        ops.push(MetadataOp::Clear(TagField::Position(field)));
    } else {
        ops.push(MetadataOp::SetPosition {
            field,
            number,
            total,
        });
    }
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
        push_built_series_op(ops, series_value, TextField::Series);
    }

    if metadata.series_part.is_some() || metadata.subseries_part.is_some() {
        push_built_series_op(ops, series_part_value, TextField::SeriesPart);
    }
}

fn push_built_series_op(ops: &mut Vec<MetadataOp>, value: Option<String>, field: TextField) {
    match value {
        Some(value) if !value.trim().is_empty() => ops.push(MetadataOp::SetText { field, value }),
        _ => ops.push(MetadataOp::Clear(TagField::Text(field))),
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
                MetadataOp::SetText { field: f, .. } => TagField::Text(*f) == *field,
                MetadataOp::SetPosition { field: f, .. } => TagField::Position(*f) == *field,
                _ => false,
            }) {
                "set"
            } else {
                "preserve"
            };
            format!("{field}={action}")
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
