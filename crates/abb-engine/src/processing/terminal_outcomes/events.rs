use crate::processing::context::processing::ProgressEventListener;
use crate::processing::progress::EmitContext;
use crate::processing::ProgressEmitter;

fn terminal_emitter(
    progress_listener: Option<&ProgressEventListener>,
    context: EmitContext,
) -> ProgressEmitter {
    ProgressEmitter::with_context(context).with_progress_listener(progress_listener.cloned())
}

pub(in crate::processing) fn emit_terminal_failed_event(
    progress_listener: Option<&ProgressEventListener>,
    context: EmitContext,
    message: &str,
) {
    terminal_emitter(progress_listener, context).emit_terminal_failed(message);
}

pub(in crate::processing) fn emit_terminal_skipped_event(
    progress_listener: Option<&ProgressEventListener>,
    context: EmitContext,
    message: &str,
) {
    terminal_emitter(progress_listener, context).emit_terminal_skipped(message);
}

pub(in crate::processing) fn emit_terminal_cancelled_event(
    progress_listener: Option<&ProgressEventListener>,
    context: EmitContext,
    message: &str,
) {
    terminal_emitter(progress_listener, context).emit_terminal_cancelled(message);
}
