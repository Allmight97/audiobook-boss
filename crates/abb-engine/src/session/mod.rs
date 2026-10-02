//! The working session: the titles being prepared, the metadata edits made
//! to them, the lookup that helps fill those edits in, each title's audio
//! choice, output naming, submission, and the downloads it imported.
//!
//! A host sends [`SessionIntent`]s and renders [`SessionUpdate`]s. Rules live
//! in the state modules; `runtime` performs the file and network work they
//! ask for.

mod audio;
mod audio_choice;
mod exports;
mod lookup;
mod metadata_form;
mod output;
mod plans;
mod runtime;
mod staged;
mod state;
mod submission;
mod tag_cache;
mod working_set;

pub use audio::{AudioChoiceView, AudioSnapshot, TitleAudio};
pub use audio_choice::{AudioChoice, AudioChoiceFacts, AudioEdit, FaacRateControl};
pub use exports::{OutputEdits, RestartOffer};
pub use lookup::{
    LookupApplyMode, LookupQueuePosition, LookupSnapshot, LookupSource, LookupStatus, QueueStep,
};
pub use metadata_form::{
    FieldAction, FieldSnapshot, FormMode, MetadataField, MetadataFormSnapshot, SeriesPartWarning,
    SubseriesPartWarning,
};
pub use output::{OutputPreview, OutputSnapshot};
pub use plans::{SizeEstimate, TitlePlan};
pub(crate) use runtime::{Session, SessionDeps};
pub use runtime::{SessionIntent, SessionOutcome, SessionReply, SessionRun};
pub use state::{
    CoverNotice, CoverSnapshot, MetadataSnapshot, MetadataStatus, SessionUpdate, TagPreview,
};
pub use submission::{SubmissionStatus, SubmitRefusal};
pub use working_set::{
    CueChoice, InputNotice, MoveDirection, SelectionModifiers, SelectionSnapshot, SortDirection,
    TitlesSnapshot,
};
