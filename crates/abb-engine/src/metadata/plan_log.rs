use crate::metadata::MetadataWritePlan;
use abb_metadata_core::{PatchOp, TagField};

pub(crate) fn log_write_plan(plan: &MetadataWritePlan, writer: &str, artifact: &str) {
    let sort = match &plan.album_sort {
        None => "preserve",
        Some(PatchOp::Set(_)) => "set",
        Some(PatchOp::Clear) => "clear",
    };
    log_field_decisions(plan, sort, writer, artifact);
}

fn field_decisions(plan: &MetadataWritePlan) -> String {
    let decisions: Vec<_> = TagField::ALL
        .iter()
        .map(|field| {
            let action = match plan.ops.iter().find(|op| op.field() == *field) {
                Some(op) if op.clears() => "clear",
                Some(_) => "set",
                None => "preserve",
            };
            format!("{field}={action}")
        })
        .collect();
    decisions.join(",")
}

pub(crate) fn log_field_decisions(
    plan: &MetadataWritePlan,
    album_sort: &str,
    writer: &str,
    artifact: &str,
) {
    log::info!(
        "metadata_plan artifact={artifact} writer={writer} fields={} album_sort={album_sort}",
        field_decisions(plan)
    );
    if let Some(cover) = plan.cover.as_ref() {
        let (action, bytes) = match cover {
            PatchOp::Set(bytes) => ("set", bytes.as_slice()),
            PatchOp::Clear => ("clear", [].as_slice()),
        };
        log::info!("metadata_cover artifact={artifact} writer={writer} source=explicit action={action} bytes={} format={:?}",
            bytes.len(), super::cover_art::detect_cover_art_format(bytes));
    }
}

#[cfg(test)]
#[path = "plan_log_tests.rs"]
mod tests;
