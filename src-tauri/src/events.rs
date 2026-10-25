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

/// Forwards engine events to every webview.
pub struct TauriEvents(pub tauri::AppHandle);

impl TauriEvents {
    fn send<E: tauri_specta::Event + Serialize + Clone>(&self, event: E) {
        note_event_stats(E::NAME, &event);
        if let Err(error) = self.0.emit(E::NAME, event) {
            log::warn!("Failed to emit {}: {error}", E::NAME);
        }
    }
}

/// Temporary measurement. `ABB_EVENT_STATS=1` logs how many events of each
/// name left the host, and their serialized size, every five seconds.
fn note_event_stats(name: &'static str, event: &impl Serialize) {
    if std::env::var("ABB_EVENT_STATS").ok().as_deref() != Some("1") {
        return;
    }
    let bytes = serde_json::to_vec(event)
        .map(|body| body.len())
        .unwrap_or(0);
    event_stats().record(name, bytes as u64);
}

fn event_stats() -> &'static EventStats {
    use std::sync::OnceLock;
    static STATS: OnceLock<EventStats> = OnceLock::new();
    STATS.get_or_init(EventStats::start)
}

struct EventStats {
    counts: std::sync::Mutex<std::collections::BTreeMap<&'static str, EventCount>>,
}

struct EventCount {
    interval_count: u64,
    interval_bytes: u64,
    total_count: u64,
    total_bytes: u64,
}

impl EventStats {
    fn start() -> Self {
        let stats = Self {
            counts: std::sync::Mutex::new(std::collections::BTreeMap::new()),
        };
        std::thread::spawn(|| loop {
            std::thread::sleep(std::time::Duration::from_secs(5));
            event_stats().log_interval();
        });
        stats
    }

    fn record(&self, name: &'static str, bytes: u64) {
        let mut counts = self
            .counts
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let entry = counts.entry(name).or_insert(EventCount {
            interval_count: 0,
            interval_bytes: 0,
            total_count: 0,
            total_bytes: 0,
        });
        entry.interval_count += 1;
        entry.interval_bytes += bytes;
        entry.total_count += 1;
        entry.total_bytes += bytes;
    }

    fn log_interval(&self) {
        let mut counts = self
            .counts
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if counts.is_empty() {
            log::info!("event-stats interval=5s none");
            return;
        }
        for (name, entry) in counts.iter_mut() {
            log::info!(
                "event-stats interval=5s name={name} count={} bytes={} total_count={} total_bytes={}",
                entry.interval_count,
                entry.interval_bytes,
                entry.total_count,
                entry.total_bytes
            );
            entry.interval_count = 0;
            entry.interval_bytes = 0;
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
