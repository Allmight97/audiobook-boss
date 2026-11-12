# Tauri IPC Boundary

- Raw `invoke`, the generated `commands` and `events` values, and the Tauri
  plugin and event APIs stay in this folder; callers use `tauriClient`, whose
  `commands` unwrap every reply and throw a normalized app error on a
  refusal.
- Native-dialog methods select files and folders only; a session question
  renders an identified snapshot, because a native question cannot be
  dismissed when the frontend is replaced.
- Covers never cross IPC as bytes: `coverSrc.ts` turns an engine cover
  address into an `abb-cover` URL the host serves.
- Name commands and types for the product, without `_v1`, `_v2`, or `_cmd`
  suffixes; a breaking change gets a new product-meaningful name.
- Derive user error messages and cancellation in `appError.ts`.
