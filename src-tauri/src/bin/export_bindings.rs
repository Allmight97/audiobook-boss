use std::path::PathBuf;

#[path = "export_bindings/recording.rs"]
mod recording;
#[path = "export_bindings/scenarios.rs"]
mod scenarios;
#[path = "export_bindings/wav.rs"]
mod wav;

fn main() {
    let output = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../src/lib/generated/tauri.ts")
        });
    audiobook_boss_lib::ipc_contract::export_typescript_bindings(&output)
        .expect("failed to export tauri-specta TypeScript bindings");
    let fixtures = output
        .parent()
        .expect("bindings output has a parent folder")
        .join("fixtures");
    recording::export_fresh(&fixtures);
    scenarios::export_scenarios(&fixtures);
}
