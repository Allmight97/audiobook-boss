use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use abb_engine::app_settings::{SettingsIntent, SettingsReply, SettingsSnapshot};
use abb_engine::session::{SessionIntent, SessionReply, SessionUpdate};
use abb_engine::{AppError, AppErrorEnvelope, MetadataIntentPatch};
use serde::Serialize;

use crate::commands::{CommandResult, EngineState};
use crate::intent_order::IntentOrder;

/// Orders the attached frontend's intents. Session and settings intents are
/// numbered separately because they change separate state.
#[derive(Default)]
pub struct FrontendLink {
    clients: AtomicU64,
    session: IntentOrder,
    settings: IntentOrder,
}

type Link<'a> = tauri::State<'a, FrontendLink>;

/// What a starting frontend needs: who it is to this host, and the state to show.
#[derive(Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FrontendAttachment {
    /// Sent with every intent so intents from an earlier frontend are refused.
    pub client: u64,
    pub session: SessionUpdate,
    pub settings: SettingsSnapshot,
}

fn replaced() -> AppErrorEnvelope {
    AppErrorEnvelope::from(&AppError::InvalidInput(
        "This window was replaced by a newer one; its request was not applied.".to_string(),
    ))
}

/// Attaches a starting frontend and returns the whole session and settings.
#[tauri::command]
#[specta::specta]
pub async fn attach_frontend(
    engine: EngineState<'_>,
    link: Link<'_>,
) -> CommandResult<FrontendAttachment> {
    let client = link.clients.fetch_add(1, Ordering::SeqCst) + 1;
    link.session.attach(client);
    link.settings.attach(client);
    Ok(FrontendAttachment {
        client,
        session: engine.session_snapshot(),
        settings: engine.settings_snapshot().await,
    })
}

/// Applies one intent to the working session and returns what changed.
/// `sequence` counts this frontend's session intents from zero; each takes
/// effect after every earlier one, in whatever order they arrive.
#[tauri::command]
#[specta::specta]
pub async fn session_dispatch(
    engine: EngineState<'_>,
    link: Link<'_>,
    client: u64,
    sequence: u64,
    intent: SessionIntent,
) -> CommandResult<SessionReply> {
    let run = {
        let _turn = link
            .session
            .turn(client, sequence)
            .await
            .ok_or_else(replaced)?;
        engine.session_begin(intent)
    };
    Ok(run.finish().await)
}

/// Applies one intent to the settings and returns the settings in effect.
/// Settings intents run one at a time in `sequence` order.
#[tauri::command]
#[specta::specta]
pub async fn settings_dispatch(
    engine: EngineState<'_>,
    link: Link<'_>,
    client: u64,
    sequence: u64,
    intent: SettingsIntent,
) -> CommandResult<SettingsReply> {
    let _turn = link
        .settings
        .turn(client, sequence)
        .await
        .ok_or_else(replaced)?;
    Ok(engine.settings_dispatch(intent).await)
}

/// The cover image the session currently shows.
#[tauri::command]
#[specta::specta]
pub fn session_cover_art(engine: EngineState<'_>) -> Option<Vec<u8>> {
    engine.session_cover_art()
}

/// The pending metadata edits for `file_paths`, as processing takes them.
#[tauri::command]
#[specta::specta]
pub fn session_metadata_intents(
    engine: EngineState<'_>,
    file_paths: Vec<String>,
) -> HashMap<String, MetadataIntentPatch> {
    engine.session_metadata_intents(&file_paths)
}
