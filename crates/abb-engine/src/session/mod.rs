//! The working session: the titles being prepared, the metadata edits made
//! to them, and the lookup that helps fill those edits in.
//!
//! A host sends [`SessionIntent`]s and renders [`SessionUpdate`]s. Rules live
//! in the state modules; `runtime` performs the file and network work they
//! ask for.

mod lookup;
mod metadata_form;
mod runtime;
mod state;
mod tag_cache;
mod working_set;

pub use lookup::{
    LookupApplyMode, LookupQueuePosition, LookupSnapshot, LookupSource, LookupStatus, QueueStep,
};
pub use metadata_form::{
    FieldAction, FieldSnapshot, FormMode, MetadataField, MetadataFormSnapshot, SeriesPartWarning,
    SubseriesPartWarning,
};
pub(crate) use runtime::{Session, SessionDeps};
pub use runtime::{SessionIntent, SessionOutcome, SessionReply, SessionRun};
pub use state::{
    CoverNotice, CoverSnapshot, DeferredWriteSnapshot, DeferredWriteState, MetadataSnapshot,
    MetadataStatus, SessionUpdate,
};
pub use working_set::{
    CueChoice, InputNotice, MoveDirection, SelectionModifiers, SelectionSnapshot, SortDirection,
    TitlesSnapshot,
};
