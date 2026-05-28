use crate::processing::{OperationKind, ProgressEmitter};

pub(in crate::processing) fn emit_terminal_failed_event(
    window: &tauri::Window,
    operation_kind: OperationKind,
    input_index: Option<usize>,
    job_id: Option<&str>,
    message: &str,
) {
    let emitter = ProgressEmitter::with_context(
        window.clone(),
        operation_kind,
        job_id.map(|value| value.to_string()),
        input_index,
    );
    emitter.emit_terminal_failed(message);
}

pub(in crate::processing) fn emit_terminal_skipped_event(
    window: &tauri::Window,
    operation_kind: OperationKind,
    input_index: Option<usize>,
    job_id: Option<&str>,
    message: &str,
) {
    let emitter = ProgressEmitter::with_context(
        window.clone(),
        operation_kind,
        job_id.map(|value| value.to_string()),
        input_index,
    );
    emitter.emit_terminal_skipped(message);
}
