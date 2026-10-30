use super::output_to_open;
use crate::processing::{ProcessCommandResult, ProcessResultEntry, ProcessResultStatus};

fn result(entries: &[(ProcessResultStatus, &str)]) -> ProcessCommandResult {
    ProcessCommandResult::new(
        entries
            .iter()
            .enumerate()
            .map(|(input_index, (status, path))| ProcessResultEntry {
                input_index,
                status: *status,
                message: String::new(),
                error: None,
                output_path: (!path.is_empty()).then(|| path.to_string()),
                preview_actual_seconds: None,
                supplemental_warning: None,
                warnings: Vec::new(),
                job_id: None,
            })
            .collect(),
    )
}

#[test]
fn only_the_single_output_of_an_uncancelled_preview_opens() {
    use ProcessResultStatus::{Failed, Success};
    let one = result(&[(Success, "/p/a.m4b")]);
    assert_eq!(output_to_open(&one, false).as_deref(), Some("/p/a.m4b"));
    assert_eq!(
        output_to_open(&one, true),
        None,
        "a cancelled run opens nothing"
    );
    let two = result(&[(Success, "/p/a.m4b"), (Success, "/p/b.m4b")]);
    assert_eq!(
        output_to_open(&two, false),
        None,
        "several outputs open nothing"
    );
    let partly = result(&[(Success, "/p/a.m4b"), (Failed, "")]);
    assert_eq!(output_to_open(&partly, false).as_deref(), Some("/p/a.m4b"));
    assert_eq!(output_to_open(&result(&[(Failed, "")]), false), None);
}
