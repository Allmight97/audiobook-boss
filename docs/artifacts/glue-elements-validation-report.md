# Glue elements — validation report

Generated from an agent review session. This documents backend and TS-boundary “glue” surfaces, what was verified in-tree, and where code smells like a temporary toolchain workaround vs intentional seam.

## Scope

- Glue inventory aligned with `docs/artifacts/audio-processing-system-map.html`, `docs/api-map.md`, and `docs/system-map.md`.
- Focus: IPC registration, command facades, processing orchestration, processor adapter, job registry, metadata bridges, and `src/lib/tauri` boundary adapters.

## Validation method

- Read `src-tauri/src/ipc_contract.rs`, `docs/api-map.md`, `src/lib/tauri/client.ts`, `src-tauri/src/processing/run.rs` / `terminal_outcomes.rs` (headers), `src-tauri/src/metadata/mod.rs`, `src-tauri/src/audio/processor/adapter.rs`, `src-tauri/src/bin/export_bindings.rs`, `src/ui/statusPanel/processing.ts` (header).
- Grepped glue-heavy paths for `TODO` / `FIXME` / `HACK` / `workaround` / `shim` (essentially none in those layers).
- Line counts for oversized modules (`wc`).

**Not done in this pass:** `scripts/checks.sh standard`, exhaustive bind of every HTML diagram filename to disk, or full binding drift automation.

## Glue elements — validated vs patchy / improvable

| Glue element | Validated against code | Temporary / lazy / better-alternative signals |
|--------------|-------------------------|-----------------------------------------------|
| **`ipc_contract.rs`** — command/event registration + **`trim_generated_typescript`** | Yes — registration matches **`api-map.md`** command list; trim runs after export | **Post-processing generated TS** (strip injected `TAURI_CHANNEL` block, trim trailing whitespace) is **toolchain workaround glue**. A cleaner fix would be upstream specta/ts export behavior or a single disciplined codegen step so the file does not need string surgery. |
| **`src-tauri/src/bin/export_bindings.rs`** | Yes — one-line call into `export_typescript_bindings` | Not a patch; thin entry point. |
| **`src-tauri/src/lib.rs`** — plugins, `JobRegistry::manage`, specta mount, window setup | Yes — matches described bootstrap | **Broad `pub use metadata::*` for tests/integration** is explicit export glue (documented in-file). Alternatives exist (narrower test-only API); not “lazy,” just a tradeoff. |
| **`commands/audio.rs`** — `#[tauri::command]` → `audio::*` / **`processing::run`** | Yes — delegates to `run::preflight_payload` / `run::process_payload` | **`list_available_encoders` uses emoji + noisy `log::info!`** — debug-style logging left in the command path; polish/noise rather than a correctness hack. |
| **`commands/metadata.rs`** (~359 LOC) | Size only (`wc`) | **Large for a command module** per repo shape triggers — risk of **mixed validation + delegation** in one file; refactor candidate, not necessarily a temporary patch. |
| **`commands/metadata_lookup/`** | Structure implied by **`api-map.md`** | Inherent **provider/router glue** for online lookup; “better” usually means **clear provider boundaries** as providers grow, not removing the seam. |
| **`processing/run.rs`** (~1026 LOC) | Read start of file | **Orchestrator sprawl** — no debt markers found, but size **violates the spirit** of `src-tauri/AGENTS.md` module targets; improvement is **extract phase helpers / job dispatch**, not deleting glue. |
| **`processing/plan.rs`** (~652 LOC) | Not fully read | Same **size/cohesion** concern as `run.rs`. |
| **`processing/terminal_outcomes.rs`** (~758 LOC) | Read top ~120 LOC | Heavy **terminal-truth classification** — reads as **deliberate domain logic** co-located with orchestration, not a quick hack. Still, **splitting** could reduce cross-cutting edits. |
| **`processing/types.rs`** | Referenced throughout | **Healthy DTO glue** between IPC and processing. |
| **`audio/processor/adapter.rs`** — `ResolvedProcessorAdapter` | Yes — read file | **Appropriate strategy seam** (native vs external FDK); no markers suggesting a shortcut. |
| **`processing/job_registry/`** | Prior exploration + **`api-map.md`** | **Lifecycle seam** — appropriate; complexity is **inherent** to concurrency/cancel. |
| **`metadata/mp4ameta_bridge.rs` + `ffmpeg_dict.rs`** | Partial read / module layout | **Dual paths** (mp4ameta vs FFmpeg dict) are **interop glue by design** (`metadata/mod.rs` comments). Not “lazy” unless one path is unmaintained (no evidence from grep). |
| **`src/lib/tauri/client.ts` + `normalizers.ts`** | Yes — read | **`normalizeNullish` / `denormalize*`** + separate **`types/*`** vs generated shapes — **structural friction** from Rust `Option`/TS `null`/`undefined`. Documented as the boundary (`normalizers.ts` header). **Possible long-term improvement**: tighter codegen or fewer parallel type universes; current layer is **explicit, not a sneak patch**. |
| **`client.ts` `listen`** — specta `generatedEvents` for progress/queue, **`tauriListen`** for other names | Yes | **`api-map.md` “raw event listen fallback”** is slightly misleading: code uses **typed specta listeners** for the two processing events and **generic listen** for everything else — **two deliberate channels**, not an error fallback. |
| **`src/ui/statusPanel/processing.ts`** (`startProcessing` orchestration) | Read header + wc ~306 LOC | **UI orchestration glue** wiring many modules — appropriate role; **risk is drift** with backend summaries (e.g. batch outcome text vs `ProcessCommandResult.summary`) if both sides evolve independently. |

## Bottom line

- **Nothing in these glue layers screamed `FIXME`/`TODO` hack** from markers; the **main “patch-like” smell** is **`trim_generated_typescript`** in **`ipc_contract.rs`** (string-level repair of generated output).
- The **largest improvement levers** are **structural**: **split oversized** `run.rs` / `plan.rs` / `terminal_outcomes.rs` / possibly **`commands/metadata.rs`**, and **reduce reliance on post-export string trimming** if/when tooling allows.
- **Dual metadata engines** and **TS normalizers** are **heavy glue**, but they match **documented boundary rules** — better labeled **intentional seams** than temporary wedges unless you choose to collapse type/codegen strategy.

## Machine-verified parity (follow-up)

For bind-of-record doc↔code parity (every diagram box and every command name), run **`scripts/checks.sh standard`** / binding checks from the repo root after substantive boundary edits.
