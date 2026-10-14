use std::sync::atomic::{AtomicU64, Ordering};

use abb_engine::app_settings::{SettingsIntent, SettingsReply, SettingsSnapshot};
use abb_engine::session::{SessionIntent, SessionOutcome, SessionReply, SessionUpdate};
use abb_engine::{AppError, AppErrorCategory, AppErrorCode, AppErrorEnvelope};
use serde::Serialize;

use crate::commands::{CommandResult, EngineState};
use crate::intent_order::{IntentOrder, Refused};

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

fn refused(reason: Refused) -> AppErrorEnvelope {
    let message = match reason {
        Refused::Replaced => {
            "This window was replaced by a newer one; its request was not applied."
        }
        Refused::Late => {
            "This request arrived after later ones had already run, so it was not applied."
        }
    };
    AppErrorEnvelope::from(&AppError::InvalidInput(message.to_string()))
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
        let _turn = link.session.turn(client, sequence).await.map_err(refused)?;
        engine.session_begin(intent)
    };
    Ok(run.finish().await)
}

/// Claims a finished preview's output, numbered with the session intents, and
/// opens it with the user's default app. The path never comes from the webview.
#[tauri::command]
#[specta::specta]
pub async fn open_preview_output(
    app: tauri::AppHandle,
    engine: EngineState<'_>,
    link: Link<'_>,
    client: u64,
    sequence: u64,
    run_id: String,
) -> CommandResult<SessionReply> {
    use tauri_plugin_opener::OpenerExt;

    let run = {
        let _turn = link.session.turn(client, sequence).await.map_err(refused)?;
        engine.session_begin(SessionIntent::TakePreviewOutput { run_id })
    };
    let mut reply = run.finish().await;
    if let SessionOutcome::PreviewOutput { path: Some(path) } = &reply.outcome {
        if let Err(error) = app.opener().open_path(path, None::<&str>) {
            reply.outcome = SessionOutcome::Rejected {
                error: AppErrorEnvelope::new(
                    AppErrorCode::InternalError,
                    AppErrorCategory::Internal,
                    error.to_string(),
                    None,
                ),
            };
        }
    }
    Ok(reply)
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
        .map_err(refused)?;
    Ok(engine.settings_dispatch(intent).await)
}
