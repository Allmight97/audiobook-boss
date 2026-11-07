use super::*;

#[test]
fn sanitized_fields_flattens_and_bounds_input() {
    let entry = FrontendLogEntry {
        level: FrontendLogLevel::Error,
        scope: "s".repeat(600),
        message: format!("line1\nline2\t{}", "m".repeat(600)),
    };

    let (scope, message) = sanitized_fields(&entry);
    let truncated = FRONTEND_LOG_FIELD_MAX_CHARS + 1;
    assert_eq!(scope.chars().count(), truncated);
    assert_eq!(message.chars().count(), truncated);
    assert!(!message.contains('\n'));
    assert!(!message.contains('\t'));
}
