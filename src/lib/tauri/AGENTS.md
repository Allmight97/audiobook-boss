# Tauri IPC Boundary

- Raw `invoke`, the generated `commands` and `events` values, and the Tauri
  plugin and event APIs stay in this folder, so every reply is unwrapped in
  one place and every refusal throws a normalized app error.
- Native-dialog methods select files and folders only; a session question
  renders an identified snapshot, because a native question cannot be
  dismissed when the frontend is replaced.
- Name commands and types for the product, without `_v1`, `_v2`, or `_cmd`
  suffixes; a breaking change gets a new product-meaningful name.
- Derive user error messages and cancellation in `appError.ts`.
