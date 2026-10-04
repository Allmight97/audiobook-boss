use std::path::PathBuf;

use specta_typescript::Typescript;
use tauri_specta::{Builder, ErrorHandlingMode};

pub fn builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new()
        .commands(tauri_specta::collect_commands![
            crate::commands::attach_frontend,
            crate::commands::session_dispatch,
            crate::commands::settings_dispatch,
            crate::commands::session_cover_art,
            crate::commands::load_cover_art_from_url,
            crate::commands::read_audio_cover_thumbnail,
            crate::commands::get_supported_audio_import_metadata,
            crate::commands::list_work_operations,
            crate::commands::cancel_work_operation,
            crate::commands::log_frontend,
        ])
        .events(tauri_specta::collect_events![
            crate::events::WorkOperationsUpdateEvent,
            crate::events::SessionUpdateEvent,
            crate::events::SettingsUpdateEvent,
        ])
        .error_handling(ErrorHandlingMode::Result)
        // ABB's JSON IPC contract uses numbers for bounded byte sizes, timestamps,
        // counts, indices, and sequence values. They remain below JavaScript's exact
        // integer range, and frontend consumers intentionally perform number arithmetic.
        // A float field may use `#[specta(type = specta_typescript::Number)]` only when
        // its Rust owner normalizes it to a finite value and the wire contract must stay
        // non-nullable. Add JS `bigint` or lossless-float semantics only for a payload
        // that needs them.
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
