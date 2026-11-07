use super::*;
use std::path::PathBuf;

fn custom_template(template: &str) -> OutputPlan {
    let mut plan = OutputPlan::default();
    plan.set_directory("/out".to_string());
    plan.set_preset(NamingPreset::CustomTemplate);
    plan.set_template(template.to_string());
    plan
}

fn reason_for(template: &str) -> String {
    let metadata = AudiobookMetadata::default();
    let source = PathBuf::from("/books/a.m4b");
    match custom_template(template).preview(Some((&metadata, &source, AudiobookFormat::M4b))) {
        OutputPreview::Unavailable { message } => message,
        other => panic!("expected an unavailable preview, got {other:?}"),
    }
}

#[test]
fn an_unknown_template_token_is_named_with_no_error_prefix() {
    assert_eq!(
        reason_for("{foo}"),
        "Unknown template token '{foo}'. Allowed tokens: {author},{title},{series},{seriesPart},{subseries},{subseriesPart},{year}"
    );
}

#[test]
fn a_template_that_climbs_out_of_the_folder_is_refused_with_no_error_prefix() {
    assert_eq!(
        reason_for("../{title}"),
        "Custom template path traversal is not allowed"
    );
}
