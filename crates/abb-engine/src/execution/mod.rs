//! The leaf below the audio pipeline and the processing lifecycle: what one
//! title's job carries while it runs (identity, cancellation, output, progress)
//! and the gate its publication passes through. It imports nothing from
//! `crate::audio` or `crate::processing`.

mod aac_decoder;
mod cancel;
mod context;
mod job_identity;
pub(crate) mod preview_config;
mod progress;
mod publish_gate;
mod session;

pub use aac_decoder::AacDecoder;
pub use cancel::CancellationChecker;
pub(crate) use context::{OutputConfig, ProcessingContext};
pub(crate) use job_identity::JobIdentity;
pub(crate) use preview_config::PREVIEW_SECONDS;
pub(crate) use progress::{
    converting_percentage, converting_percentage_from_seconds, ProgressEmitter,
    ProgressEventListener,
};
pub use progress::{EventStage, ProgressEvent};
pub(crate) use publish_gate::{PublishGate, PublishedOutput};
pub(crate) use session::ProcessingSession;
