use abb_engine::app_settings::SettingsSnapshot;
use abb_engine::session::SessionUpdate;
use abb_engine::work_runtime::WorkOperationsUpdate;
use abb_engine::{EngineEvent, EventSink};
use serde::Serialize;
use tauri::Emitter;

#[derive(Clone, Serialize, specta::Type)]
#[serde(transparent)]
pub struct WorkOperationsUpdateEvent(pub WorkOperationsUpdate);

impl tauri_specta::Event for WorkOperationsUpdateEvent {
    const NAME: &'static str = "work-operations-update";
}

#[derive(Clone, Serialize, specta::Type)]
#[serde(transparent)]
pub struct SessionUpdateEvent(pub SessionUpdate);

impl tauri_specta::Event for SessionUpdateEvent {
    const NAME: &'static str = "session-update";
}

#[derive(Clone, Serialize, specta::Type)]
#[serde(transparent)]
pub struct SettingsUpdateEvent(pub SettingsSnapshot);

impl tauri_specta::Event for SettingsUpdateEvent {
    const NAME: &'static str = "settings-update";
}

#[derive(Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct QuitPressAgainEvent {
    pub window_ms: u32,
}

impl tauri_specta::Event for QuitPressAgainEvent {
    const NAME: &'static str = "quit-press-again";
}

pub struct TauriEvents(pub tauri::AppHandle);

impl TauriEvents {
    fn send<E: tauri_specta::Event + Serialize + Clone>(&self, event: E) {
        if let Err(error) = self.0.emit(E::NAME, event) {
            log::warn!("Failed to emit {}: {error}", E::NAME);
        }
    }
}

impl EventSink for TauriEvents {
    fn emit(&self, event: EngineEvent) {
        match event {
            EngineEvent::WorkOperations(update) => self.send(WorkOperationsUpdateEvent(update)),
            EngineEvent::Session(update) => self.send(SessionUpdateEvent(update)),
            EngineEvent::Settings(snapshot) => self.send(SettingsUpdateEvent(*snapshot)),
            EngineEvent::QuitPressAgain { window_ms } => {
                self.send(QuitPressAgainEvent { window_ms });
            }
        }
    }
}
