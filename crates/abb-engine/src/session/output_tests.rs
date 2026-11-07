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

fn assert_no_error_prefix(reason: &str) {
    for prefix in ["Invalid input", "File validation failed"] {
        assert!(!reason.starts_with(prefix), "{reason}");
    }
}

#[test]
fn an_unknown_template_token_is_named_with_no_error_prefix() {
    let reason = reason_for("{foo}");

    assert_no_error_prefix(&reason);
    assert!(reason.contains("{foo}"), "{reason}");
}

#[test]
fn a_template_that_climbs_out_of_the_folder_is_refused_with_no_error_prefix() {
    let reason = reason_for("../{title}");

    assert_no_error_prefix(&reason);
    assert!(reason.contains("traversal"), "{reason}");
}
