//! The working session: the titles being prepared, the metadata edits made
//! to them, and the lookup that helps fill those edits in.
//!
//! A host sends [`SessionIntent`]s and renders [`SessionUpdate`]s. Rules live
//! in the state modules; `runtime` performs the file and network work they
//! ask for.

mod audio;
mod audio_choice;
mod lookup;
mod metadata_form;
mod output;
mod plans;
mod runtime;
mod state;
mod tag_cache;
mod working_set;

pub use audio::{AudioChoiceView, AudioSnapshot, TitleAudio};
pub use audio_choice::{AudioChoice, AudioChoiceFacts, AudioEdit, FaacRateControl};
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
pub use working_set::{
    CueChoice, InputNotice, MoveDirection, SelectionModifiers, SelectionSnapshot, SortDirection,
    TitlesSnapshot,
};
