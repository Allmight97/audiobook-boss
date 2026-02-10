fn main() {
    audiobook_boss_lib::ipc_contract::export_typescript_bindings()
        .expect("failed to export tauri-specta TypeScript bindings");
}
