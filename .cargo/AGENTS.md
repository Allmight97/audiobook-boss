# Cargo config

## Accepted advisories (`audit.toml`)

- The GTK3 crates (RUSTSEC-2024-0411 to 0420 and 0429) come from tauri, wry,
  and webkit2gtk on Linux. Their warnings are unmaintained or unsound only,
  and nothing replaces them until tauri and wry migrate.
- RUSTSEC-2024-0370 and 0436 are proc-macro crates reached through
  gtk3-macros, glib-macros, and specta; they run at compile time only.
- RUSTSEC-2026-0370 to 0372 are the zbus 5 chain through keepawake,
  secret-service, and the keyring store on Linux. They are unmaintained only;
  the successor is zbus 6, and they stay until those three crates move to it.
  Dropping the keyring store would lose saved Audible sign-ins on every
  restart.
- RUSTSEC-2026-0543 (chrono, unmaintained only) reaches the app only through
  tauri-utils and serde_with; ABB does not use chrono, and it stays until
  tauri drops it.
