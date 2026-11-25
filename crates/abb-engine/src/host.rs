use std::sync::Arc;

use crate::app_settings::SettingsSnapshot;
use crate::power::{ActiveWork, PowerManager};
use crate::session::SessionUpdate;
use crate::work_runtime::WorkOperationsUpdate;

#[expect(
    clippy::large_enum_variant,
    reason = "Each event is built once and moved to the sink; boxing would only add an allocation"
)]
#[derive(Clone)]
pub enum EngineEvent {
    WorkOperations(WorkOperationsUpdate),
    Session(SessionUpdate),
    Settings(Box<SettingsSnapshot>),
    QuitPressAgain { window_ms: u32 },
}

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

    pub(crate) fn power(&self) -> PowerManager {
        self.power.clone()
    }
}
