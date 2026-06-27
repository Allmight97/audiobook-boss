use crate::processing::context::processing::ProgressEventListener;
use crate::processing::progress::EmitContext;
use crate::processing::ProgressEmitter;

/// Builds a terminal-event emitter honoring the foreground/background split:
/// background operations (which carry a progress listener) report through
/// snapshots, foreground operations emit to the window.
fn terminal_emitter(
    window: &tauri::Window,
    progress_listener: Option<&ProgressEventListener>,
    context: EmitContext,
) -> ProgressEmitter {
    let window = if progress_listener.is_some() {
        None
    } else {
        Some(window.clone())
    };
    ProgressEmitter::with_context(window, context)
        .with_progress_listener(progress_listener.cloned())
}

pub(in crate::processing) fn emit_terminal_failed_event(
    window: &tauri::Window,
    progress_listener: Option<&ProgressEventListener>,
    context: EmitContext,
    message: &str,
) {
    terminal_emitter(window, progress_listener, context).emit_terminal_failed(message);
}

pub(in crate::processing) fn emit_terminal_skipped_event(
    window: &tauri::Window,
    progress_listener: Option<&ProgressEventListener>,
    context: EmitContext,
    message: &str,
) {
    terminal_emitter(window, progress_listener, context).emit_terminal_skipped(message);
}
