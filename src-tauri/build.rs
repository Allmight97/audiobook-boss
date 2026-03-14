use std::env;

fn main() {
    enforce_supported_macos_target();
    tauri_build::build()
}

fn enforce_supported_macos_target() {
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return;
    }

    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_else(|_| "unknown".to_string());
    if target_arch != "aarch64" {
        panic!(
            "AudioBook Boss supports only Apple Silicon macOS builds. Refusing target architecture '{}'.",
            target_arch
        );
    }

    let host = env::var("HOST").unwrap_or_default();
    if host.ends_with("apple-darwin") && !host.starts_with("aarch64-") {
        panic!(
            "AudioBook Boss requires a native Apple Silicon Rust toolchain on macOS. Refusing host '{}'.",
            host
        );
    }
}
