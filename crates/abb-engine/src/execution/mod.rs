//! The leaf below the audio pipeline and the processing lifecycle: what one
//! title's job carries while it runs (identity, cancellation, output, progress)
//! and the gate its publication passes through. It takes no module above the
//! audio pipeline except `audio::AacDecoder`, as a type.

mod cancel;
mod context;
mod job_identity;
pub(crate) mod preview_config;
mod progress;
mod publish_gate;
mod session;

#[cfg(test)]
pub(crate) use cancel::cancelled_token;
pub use cancel::CancellationChecker;
pub(crate) use context::{OutputConfig, ProcessingContext};
pub(crate) use job_identity::JobIdentity;
pub(crate) use preview_config::PREVIEW_SECONDS;
pub(crate) use progress::{
    converting_percentage, converting_percentage_from_seconds, EmitContext, ProgressEmitter,
    ProgressEventListener,
};
pub use progress::{EventStage, ProgressEvent};
pub(crate) use publish_gate::{PublishGate, PublishedOutput};
pub(crate) use session::ProcessingSession;
