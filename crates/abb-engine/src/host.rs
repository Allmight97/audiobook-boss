//! What the engine asks of the process that hosts it.
//!
//! A host supplies an [`EventSink`] and the directories in [`crate::EngineConfig`].
//! Everything else (job scheduling, keep-awake, operation state) is engine-owned.

use std::sync::Arc;

use crate::app_settings::SettingsSnapshot;
use crate::power::{ActiveWork, PowerManager};
use crate::processing::{ProgressEvent, QueueEvent};
use crate::remote_source::AcquisitionJob;
use crate::session::SessionUpdate;
use crate::work_runtime::{OperationListSnapshot, OperationSnapshot};

/// A fact the engine publishes without being asked. Hosts forward each to
/// their UI; a host with no UI may ignore them.
// Each event is built once and moved to the sink; boxing would only add an allocation.
#[allow(clippy::large_enum_variant)]
#[derive(Clone)]
pub enum EngineEvent {
    /// Progress of a direct preview run.
    ProcessingProgress(ProgressEvent),
    /// The queue of a direct preview run.
    ProcessingQueue(QueueEvent),
    /// One accepted operation changed.
    WorkOperationSnapshot(OperationSnapshot),
    /// The set of accepted operations changed.
    WorkOperationList(OperationListSnapshot),
    /// The working session changed without the host asking, or before a
    /// requested change finished.
    Session(SessionUpdate),
    /// The settings changed because of something other than a settings
    /// intent, such as a default chosen in the session.
    Settings(Box<SettingsSnapshot>),
    /// A remote-source acquisition progressed, finished, or handed its files
    /// to the session.
    Acquisition(Box<AcquisitionJob>),
}

/// Receives engine events. Called from engine worker threads, so an
/// implementation must not block.
pub trait EventSink: Send + Sync {
    fn emit(&self, event: EngineEvent);
}

/// An event sink that drops every event, for hosts and tests with no UI.
pub struct DiscardEvents;

impl EventSink for DiscardEvents {
    fn emit(&self, _event: EngineEvent) {}
}

/// The engine-internal link a running operation holds: where its events go and
/// the keep-awake owner it acquires while it works.
#[derive(Clone)]
pub(crate) struct Host {
    events: Arc<dyn EventSink>,
    power: PowerManager,
}

impl Host {
    pub(crate) fn new(events: Arc<dyn EventSink>, power: PowerManager) -> Self {
        Self { events, power }
    }

    pub(crate) fn emit(&self, event: EngineEvent) {
        self.events.emit(event);
    }

    pub(crate) fn begin_active_work(&self) -> ActiveWork {
        self.power.begin()
    }
}
