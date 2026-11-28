use crate::metadata::field_schema::{position_keys, text_keys, KeySet};
use crate::metadata::publication_year_from_date;
use crate::metadata::MetadataWritePlan;
use abb_metadata_core::{FieldOp, PatchOp, TextField};

pub(crate) fn tag_pairs_for_ops(ops: &[FieldOp]) -> Vec<(&'static str, String)> {
    let mut pairs = Vec::new();
    for op in ops {
        match op {
            FieldOp::Text {
                field,
                op: PatchOp::Set(value),
            } => {
                for key in text_keys(*field).write {
                    pairs.push((*key, value.clone()));
                }
                if *field == TextField::Date {
                    if let Some(year) = publication_year_from_date(Some(value)) {
                        pairs.push(("year", year.to_string()));
                    }
                }
            }
            FieldOp::Position {
                field,
                op: PatchOp::Set((number, total)),
            } => pairs.push((
                position_keys(*field).write,
                format_position_field(*number, *total),
            )),
            FieldOp::Text { .. } | FieldOp::Position { .. } => {}
        }
    }
    pairs.push(("media_type", "2".to_string()));
    pairs
}

pub(crate) struct ExistingKeyRemoval {
    dropped: Vec<KeySet>,
}

impl ExistingKeyRemoval {
    pub(crate) fn for_plan(plan: &MetadataWritePlan) -> Self {
        let dropped = plan
            .ops
            .iter()
            .filter_map(|op| match op {
                FieldOp::Text {
                    field,
                    op: PatchOp::Set(_),
                } => Some(text_keys(*field).drop_on_set),
                FieldOp::Text {
                    field,
                    op: PatchOp::Clear,
                } => Some(text_keys(*field).drop_on_clear),
                FieldOp::Position {
                    field,
                    op: PatchOp::Clear,
                } => Some(position_keys(*field).drop_on_clear),
                FieldOp::Position { .. } => None,
            })
            .collect();
        Self { dropped }
    }

    pub(crate) fn removes(&self, key: &str) -> bool {
        self.dropped.iter().any(|set| set.contains(key))
    }
}

fn format_position_field(number: u32, total: Option<u32>) -> String {
    match total {
        Some(total) => format!("{number}/{total}"),
        None => number.to_string(),
    }
}
