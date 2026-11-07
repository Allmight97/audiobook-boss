use crate::{
    apply_shared_metadata_patch_fields, build_series_list, validate_series_family_if_touched,
    AlbumSortPatchOp, AudiobookMetadata, MetadataIntentPatch, PatchOp, PositionField, Result,
    TagField, TextField,
};

type Position = (u32, Option<u32>);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldOp {
    Text {
        field: TextField,
        op: PatchOp<String>,
    },
    Position {
        field: PositionField,
        op: PatchOp<Position>,
    },
}

impl FieldOp {
    pub fn field(&self) -> TagField {
        match self {
            FieldOp::Text { field, .. } => TagField::Text(*field),
            FieldOp::Position { field, .. } => TagField::Position(*field),
        }
    }

    pub fn clears(&self) -> bool {
        match self {
            FieldOp::Text { op, .. } => matches!(op, PatchOp::Clear),
            FieldOp::Position { op, .. } => matches!(op, PatchOp::Clear),
        }
    }
}

/// What a tag write does to a file, in write order. A field with no op keeps
/// the file's value. `ops` are projected already: the series fields hold the
/// joined primary and sub-series value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetadataWritePlan {
    pub ops: Vec<FieldOp>,
    pub album_sort: Option<PatchOp<String>>,
    pub cover: Option<PatchOp<Vec<u8>>>,
    pub series_touched: bool,
}

impl MetadataWritePlan {
    /// The plan for a validated edit of a file whose tags read `source`.
    pub fn from_patch(patch: &MetadataIntentPatch, source: AudiobookMetadata) -> Result<Self> {
        let patch = patch.normalized_or_error()?;
        let series_touched = patch.touches_series_family();

        let mut ops = Vec::new();
        for field in plain_text_fields() {
            match patch.text(field) {
                Some(PatchOp::Set(value)) => ops.push(text_op(field, value)),
                Some(PatchOp::Clear) => ops.push(FieldOp::Text {
                    field,
                    op: PatchOp::Clear,
                }),
                None => {}
            }
        }
        if series_touched {
            let mut effective = source;
            apply_shared_metadata_patch_fields(&patch, &mut effective);
            validate_series_family_if_touched(&effective, true)?;
            ops.extend(series_ops(
                &effective,
                patch.series.is_some() || patch.subseries.is_some(),
                patch.series_part.is_some() || patch.subseries_part.is_some(),
            ));
        }
        for field in PositionField::ALL {
            match patch.position(field) {
                Some(PatchOp::Set(position)) => ops.push(position_op(field, *position)),
                Some(PatchOp::Clear) => ops.push(FieldOp::Position {
                    field,
                    op: PatchOp::Clear,
                }),
                None => {}
            }
        }

        Ok(Self {
            ops,
            album_sort: patch.album_sort.as_ref().map(|op| match op {
                AlbumSortPatchOp::Set(value) => sort_op(value),
                AlbumSortPatchOp::Clear => PatchOp::Clear,
            }),
            cover: patch.cover_art.as_ref().map(|op| match op {
                PatchOp::Set(bytes) => cover_op(bytes),
                PatchOp::Clear => PatchOp::Clear,
            }),
            series_touched,
        })
    }

    /// The plan for tags read off a real file or built by processing, where a
    /// field is blank, zero or empty when it is cleared: blank text clears,
    /// a date is trimmed (other text is written as found), a track or disc
    /// number 0 clears, an empty cover clears.
    pub fn from_metadata(metadata: &AudiobookMetadata) -> Self {
        let mut ops = Vec::new();
        for field in plain_text_fields() {
            if let Some(value) = metadata.text(field) {
                ops.push(text_op(field, value));
            }
        }
        let series_touched = metadata.series.is_some()
            || metadata.series_part.is_some()
            || metadata.subseries.is_some()
            || metadata.subseries_part.is_some();
        ops.extend(series_ops(
            metadata,
            metadata.series.is_some() || metadata.subseries.is_some(),
            metadata.series_part.is_some() || metadata.subseries_part.is_some(),
        ));
        for field in PositionField::ALL {
            if let Some(position) = metadata.position(field) {
                ops.push(position_op(field, position));
            }
        }

        Self {
            ops,
            album_sort: metadata.album_sort.as_deref().map(sort_op),
            cover: metadata.cover_art.as_deref().map(cover_op),
            series_touched,
        }
    }
}

/// The text fields whose tag value is the field itself; the series fields
/// join the primary and sub-series (`series_ops`).
fn plain_text_fields() -> impl Iterator<Item = TextField> {
    TextField::ALL
        .into_iter()
        .filter(|field| !matches!(field, TextField::Series | TextField::SeriesPart))
}

fn text_op(field: TextField, value: &str) -> FieldOp {
    let op = if value.trim().is_empty() {
        PatchOp::Clear
    } else if field == TextField::Date {
        PatchOp::Set(value.trim().to_string())
    } else {
        PatchOp::Set(value.to_string())
    };
    FieldOp::Text { field, op }
}

fn position_op(field: PositionField, position: Position) -> FieldOp {
    let op = if position.0 == 0 {
        PatchOp::Clear
    } else {
        PatchOp::Set(position)
    };
    FieldOp::Position { field, op }
}

fn sort_op(value: &str) -> PatchOp<String> {
    if value.trim().is_empty() {
        PatchOp::Clear
    } else {
        PatchOp::Set(value.to_string())
    }
}

fn cover_op(bytes: &[u8]) -> PatchOp<Vec<u8>> {
    if bytes.is_empty() {
        PatchOp::Clear
    } else {
        PatchOp::Set(bytes.to_vec())
    }
}

fn series_ops(
    metadata: &AudiobookMetadata,
    names_touched: bool,
    parts_touched: bool,
) -> Vec<FieldOp> {
    let (series, series_part) = build_series_list(
        metadata.series.as_deref(),
        metadata.series_part.as_deref(),
        metadata.subseries.as_deref(),
        metadata.subseries_part.as_deref(),
    );
    let mut ops = Vec::new();
    for (touched, field, value) in [
        (names_touched, TextField::Series, series),
        (parts_touched, TextField::SeriesPart, series_part),
    ] {
        if touched {
            ops.push(FieldOp::Text {
                field,
                op: value.map_or(PatchOp::Clear, PatchOp::Set),
            });
        }
    }
    ops
}

#[cfg(test)]
#[path = "write_plan_tests.rs"]
mod tests;
