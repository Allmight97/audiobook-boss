//! The engine's events as Tauri events. Names and payload shapes here are the
//! frontend contract; `ipc_contract.rs` registers them for binding generation.

use abb_engine::app_settings::SettingsSnapshot;
use abb_engine::session::SessionUpdate;
use abb_engine::work_runtime::WorkOperationsUpdate;
use abb_engine::{EngineEvent, EventSink};
use serde::Serialize;
use tauri::Emitter;

// Rust does not allow implementing `tauri_specta::Event` here for a type the
// engine defines, so each engine payload gets a host-owned event type.

/// An accepted operation changed, with the display order as of then.
#[derive(Clone, Serialize, specta::Type)]
#[serde(transparent)]
pub struct WorkOperationsUpdateEvent(pub WorkOperationsUpdate);

impl tauri_specta::Event for WorkOperationsUpdateEvent {
    const NAME: &'static str = "work-operations-update";
}

/// What changed in the working session without the frontend asking, or
/// before a change it asked for finished.
#[derive(Clone, Serialize, specta::Type)]
#[serde(transparent)]
pub struct SessionUpdateEvent(pub SessionUpdate);

impl tauri_specta::Event for SessionUpdateEvent {
    const NAME: &'static str = "session-update";
}

/// The settings after a change made outside a settings intent.
#[derive(Clone, Serialize, specta::Type)]
#[serde(transparent)]
pub struct SettingsUpdateEvent(pub SettingsSnapshot);

impl tauri_specta::Event for SettingsUpdateEvent {
    const NAME: &'static str = "settings-update";
}

/// Tells the frontend the OS asked ABB to open files; it then drains the queue.
#[derive(Clone, Default, Serialize, specta::Type)]
pub struct OpenedAudioFilesEvent {}

impl tauri_specta::Event for OpenedAudioFilesEvent {
    const NAME: &'static str = "opened-audio-files";
}

/// Forwards engine events to every webview.
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
        }
    }
}
