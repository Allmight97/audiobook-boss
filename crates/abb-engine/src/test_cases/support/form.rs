use crate::session::{FieldSnapshot, MetadataField, MetadataFormSnapshot};

pub(crate) fn field_of(form: &MetadataFormSnapshot, field: MetadataField) -> FieldSnapshot {
    form.fields
        .iter()
        .find(|snapshot| snapshot.field == field)
        .cloned()
        .unwrap_or_else(|| panic!("the form has no {field:?} field"))
}
