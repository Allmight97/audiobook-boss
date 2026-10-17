use std::path::PathBuf;

fn main() {
    let output = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../src/lib/generated/tauri.ts")
        });
    audiobook_boss_lib::ipc_contract::export_typescript_bindings(&output)
        .expect("failed to export tauri-specta TypeScript bindings");
}
