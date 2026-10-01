use crate::host::Host;
use crate::processing::context::processing::ProgressEventListener;
use crate::processing::progress::EmitContext;
use crate::processing::ProgressEmitter;

/// Builds a terminal-event emitter honoring the foreground/background split:
/// background operations (which carry a progress listener) report through
/// snapshots, foreground operations emit to the host.
fn terminal_emitter(
    host: &Host,
    progress_listener: Option<&ProgressEventListener>,
    context: EmitContext,
) -> ProgressEmitter {
    let host = if progress_listener.is_some() {
        None
    } else {
        Some(host.clone())
    };
    ProgressEmitter::with_context(host, context).with_progress_listener(progress_listener.cloned())
}

pub(in crate::processing) fn emit_terminal_failed_event(
    host: &Host,
    progress_listener: Option<&ProgressEventListener>,
    context: EmitContext,
    message: &str,
) {
    terminal_emitter(host, progress_listener, context).emit_terminal_failed(message);
}

pub(in crate::processing) fn emit_terminal_skipped_event(
    host: &Host,
    progress_listener: Option<&ProgressEventListener>,
    context: EmitContext,
    message: &str,
) {
    terminal_emitter(host, progress_listener, context).emit_terminal_skipped(message);
}

pub(in crate::processing) fn emit_terminal_cancelled_event(
    host: &Host,
    progress_listener: Option<&ProgressEventListener>,
    context: EmitContext,
    message: &str,
) {
    terminal_emitter(host, progress_listener, context).emit_terminal_cancelled(message);
}
