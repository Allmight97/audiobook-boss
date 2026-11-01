use super::*;

#[test]
fn sanitized_fields_flattens_and_bounds_input() {
    let entry = FrontendLogEntry {
        level: FrontendLogLevel::Error,
        scope: "s".repeat(600),
        message: format!("line1\nline2\t{}", "m".repeat(600)),
    };

    let (scope, message) = sanitized_fields(&entry);
    assert_eq!(scope.chars().count(), 501);
    assert_eq!(message.chars().count(), 501);
    assert!(!message.contains('\n'));
    assert!(!message.contains('\t'));
}

#[test]
fn log_frontend_accepts_oversized_input() {
    assert!(log_frontend(FrontendLogEntry {
        level: FrontendLogLevel::Warn,
        scope: "s".repeat(600),
        message: "m".repeat(600),
    })
    .is_ok());
}
