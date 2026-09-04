use std::path::PathBuf;

use specta_typescript::Typescript;
use tauri_specta::{Builder, ErrorHandlingMode};

pub fn builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new()
        .commands(tauri_specta::collect_commands![
            crate::commands::get_app_settings,
            crate::commands::update_app_settings,
            crate::commands::reset_app_settings,
            crate::commands::validate_files,
            crate::commands::read_audio_metadata,
            crate::commands::write_cover_art,
            crate::commands::load_cover_art_file,
            crate::commands::load_cover_art_from_url,
            crate::commands::read_audio_cover_thumbnail,
            crate::commands::validate_metadata_intent_patch,
            crate::commands::save_metadata_to_file,
            crate::commands::metadata::save_batch::save_metadata_batch,
            crate::commands::search_online_metadata,
            crate::commands::analyze_audio_files,
            crate::commands::get_supported_audio_import_metadata,
            crate::commands::discover_audio_import_paths,
            crate::commands::take_opened_audio_files,
            crate::commands::list_remote_source_providers,
            crate::commands::get_remote_source_account_state,
            crate::commands::start_remote_source_auth,
            crate::commands::complete_remote_source_auth,
            crate::commands::logout_remote_source_account,
            crate::commands::load_remote_source_library,
            crate::commands::start_remote_source_acquisition,
            crate::commands::get_remote_source_acquisition_status,
            crate::commands::cancel_remote_source_acquisition,
            crate::commands::purge_remote_source_session,
            crate::commands::search_remote_source_releases,
            crate::commands::grab_remote_source_release,
            crate::commands::get_remote_source_indexer_connection,
            crate::commands::update_remote_source_indexer_connection,
            crate::commands::test_remote_source_indexer_connection,
            crate::commands::validate_encoder_settings,
            crate::commands::get_runtime_settings_capabilities,
            crate::commands::preview_output_path,
            crate::commands::preflight_processing_plan,
            crate::commands::get_max_concurrent_jobs,
            crate::commands::set_max_concurrent_jobs,
            crate::commands::process_audiobook_files,
            crate::commands::submit_processing_operation,
            crate::commands::list_work_operations,
            crate::commands::get_work_operation,
            crate::commands::cancel_work_operation,
            crate::commands::log_frontend,
        ])
        .events(tauri_specta::collect_events![
            crate::processing::ProgressEvent,
            crate::processing::QueueEvent,
            crate::opened_audio::OpenedAudioFilesEvent,
            crate::work_runtime::WorkOperationSnapshotEvent,
            crate::work_runtime::WorkOperationListSnapshotEvent
        ])
        .error_handling(ErrorHandlingMode::Result)
        // ABB's JSON IPC contract uses numbers for bounded byte sizes, timestamps,
        // counts, indices, and sequence values. They remain below JavaScript's exact
        // integer range, and frontend consumers intentionally perform number arithmetic.
        .dangerously_cast_bigints_to_number()
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

    builder().export(Typescript::default(), &output_path)?;

    let generated = std::fs::read_to_string(&output_path)?;
    let normalized = trim_generated_typescript(&generated);
    if normalized != generated {
        std::fs::write(&output_path, normalized)?;
    }

    Ok(())
}

fn trim_generated_typescript(input: &str) -> String {
    let input = input.replace(
        "\n\n// Injected by export_bindings.rs to prevent tree-shaking of TAURI_CHANNEL.\nvoid TAURI_CHANNEL;\n",
        "\n",
    );
    let mut normalized = String::with_capacity(input.len());

    for line in input.split_inclusive('\n') {
        let Some(content) = line.strip_suffix('\n') else {
            normalized.push_str(line.trim_end_matches([' ', '\t']));
            continue;
        };

        normalized.push_str(content.trim_end_matches([' ', '\t']));
        normalized.push('\n');
    }

    while normalized.ends_with("\n\n") {
        normalized.pop();
    }
    if !normalized.is_empty() && !normalized.ends_with('\n') {
        normalized.push('\n');
    }

    normalized
}

#[cfg(test)]
mod tests {
    use super::trim_generated_typescript;

    #[test]
    fn generated_typescript_has_one_terminal_newline() {
        assert_eq!(trim_generated_typescript("export {};\n\n"), "export {};\n");
        assert_eq!(trim_generated_typescript("export {};"), "export {};\n");
    }
}
