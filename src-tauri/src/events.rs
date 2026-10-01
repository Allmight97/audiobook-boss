//! The engine's events as Tauri events. Names and payload shapes here are the
//! frontend contract; `ipc_contract.rs` registers them for binding generation.

use abb_engine::processing::{ProgressEvent, QueueEvent};
use abb_engine::work_runtime::OperationSnapshot;
use abb_engine::{EngineEvent, EventSink};
use serde::Serialize;
use tauri::Emitter;

// Rust does not allow implementing `tauri_specta::Event` here for a type the
// engine defines, so each engine payload gets a host-owned event type.

#[derive(Clone, Serialize, specta::Type)]
#[serde(transparent)]
pub struct ProcessingProgressEvent(pub ProgressEvent);

impl tauri_specta::Event for ProcessingProgressEvent {
    const NAME: &'static str = "processing-progress";
}

#[derive(Clone, Serialize, specta::Type)]
#[serde(transparent)]
pub struct ProcessingQueueEvent(pub QueueEvent);

impl tauri_specta::Event for ProcessingQueueEvent {
    const NAME: &'static str = "processing-queue";
}

#[derive(Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct WorkOperationSnapshotEvent {
    pub snapshot: OperationSnapshot,
}

impl tauri_specta::Event for WorkOperationSnapshotEvent {
    const NAME: &'static str = "work-operation-snapshot";
}

#[derive(Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct WorkOperationListSnapshotEvent {
    pub membership_revision: u64,
    pub operations: Vec<OperationSnapshot>,
}

impl tauri_specta::Event for WorkOperationListSnapshotEvent {
    const NAME: &'static str = "work-operation-list-snapshot";
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
            EngineEvent::ProcessingProgress(event) => self.send(ProcessingProgressEvent(event)),
            EngineEvent::ProcessingQueue(event) => self.send(ProcessingQueueEvent(event)),
            EngineEvent::WorkOperationSnapshot(snapshot) => {
                self.send(WorkOperationSnapshotEvent { snapshot });
            }
            EngineEvent::WorkOperationList(list) => self.send(WorkOperationListSnapshotEvent {
                membership_revision: list.membership_revision,
                operations: list.operations,
            }),
        }
    }
}
