use std::path::PathBuf;

use specta_typescript::{BigIntExportBehavior, Typescript};
use tauri_specta::{Builder, ErrorHandlingMode};

pub fn builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new()
        .commands(tauri_specta::collect_commands![
            crate::commands::ping,
            crate::commands::echo,
            crate::commands::validate_files,
            crate::commands::read_audio_metadata,
            crate::commands::write_cover_art,
            crate::commands::load_cover_art_file,
            crate::commands::load_cover_art_from_url,
            crate::commands::save_metadata_to_file,
            crate::commands::search_online_metadata,
            crate::commands::analyze_audio_files,
            crate::commands::validate_encoder_settings_cmd,
            crate::commands::list_available_encoders,
            crate::commands::get_max_concurrent_jobs,
            crate::commands::set_max_concurrent_jobs,
            crate::commands::process_audiobook_files_v2,
            crate::commands::cancel_processing,
        ])
        .events(tauri_specta::collect_events![
            crate::audio::ProgressEvent,
            crate::audio::QueueEvent
        ])
        .error_handling(ErrorHandlingMode::Throw)
}

pub fn default_typescript_output_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("src")
        .join("lib")
        .join("generated")
        .join("tauri.ts")
}

pub fn export_typescript_bindings() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let output_path = default_typescript_output_path();

    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    builder().export(
        Typescript::default().bigint(BigIntExportBehavior::Number),
        &output_path,
    )?;

    let generated = std::fs::read_to_string(&output_path)?;
    if !generated.contains("void TAURI_CHANNEL;") {
        let patched = generated.replacen(
            "import * as TAURI_API_EVENT from \"@tauri-apps/api/event\";",
            "import * as TAURI_API_EVENT from \"@tauri-apps/api/event\";\nvoid TAURI_CHANNEL;",
            1,
        );
        std::fs::write(&output_path, patched)?;
    }

    Ok(())
}
