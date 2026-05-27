# Decisions

## 2026-05-26 - PR #332 Review-Fix Decisions

Basis: ABB's App Settings, JobRegistry, frontend owner, and Public API Strip ownership rules; Rust read-modify-write critical-section practice; and the existing idle-only concurrency reconfiguration invariant.

- Serialized App Settings updates inside the backend `app_settings` module instead of debouncing or batching frontend writes. App Settings owns durable settings merge/write integrity; independent UI owners should be allowed to persist narrow patches without coordinating with each other.
- Rolled back `JobRegistry` from the previous live effective concurrency after App Settings update/reset failure instead of restoring from the previous persisted preference. The live registry is the runtime authority for active concurrency, and persisted settings may be stale or invalid when file I/O is already failing.
- Kept concurrency rollback best-effort rather than introducing a generic runtime/filesystem transaction layer. Full atomicity across `JobRegistry` and settings storage would widen this PR into a broader lifecycle transaction model; the current fix keeps the App Settings boundary truthful while preserving the existing idle-only registry rule.
- Split App Settings hydration by frontend control owner. One owner failing to hydrate should not block encoder, output, or concurrency owners that can still apply their part of the same validated settings payload.
- Split accepted concurrency persistence from follow-up effective-state refresh. If backend settings acceptance succeeds but `getMaxConcurrentJobs` fails, the UI should keep the accepted selection and fall back to accepted settings rather than rolling back to a false previous state.
