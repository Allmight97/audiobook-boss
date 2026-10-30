use std::sync::Arc;

use crate::app_settings::SettingsSnapshot;
use crate::power::{ActiveWork, PowerManager};
use crate::session::SessionUpdate;
use crate::work_runtime::WorkOperationsUpdate;

/// A fact the engine publishes without being asked. Hosts forward each to
/// their UI; a host with no UI may ignore them.
#[expect(
    clippy::large_enum_variant,
    reason = "Each event is built once and moved to the sink; boxing would only add an allocation"
)]
#[derive(Clone)]
pub enum EngineEvent {
    /// An accepted operation changed; carries the display order as of then.
    WorkOperations(WorkOperationsUpdate),
    /// The working session changed without the host asking, or before a
    /// requested change finished.
    Session(SessionUpdate),
    /// The settings changed because of something other than a settings
    /// intent, such as a default chosen in the session.
    Settings(Box<SettingsSnapshot>),
}

/// Receives engine events. Called from engine worker threads, so an
/// implementation must not block.
pub trait EventSink: Send + Sync {
    fn emit(&self, event: EngineEvent);
}

pub struct DiscardEvents;

impl EventSink for DiscardEvents {
    fn emit(&self, _event: EngineEvent) {}
}

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
