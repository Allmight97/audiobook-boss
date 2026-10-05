# Audiobook Boss System Map

Read this map only for repository onboarding, unclear ownership, or a change
that crosses frontend/backend or several product owners. For an ordinary local
change, start with root `AGENTS.md`, the nearest nested `AGENTS.md`, and the
owning code and tests.

ABB is a local, single-user desktop application. A Rust engine
(`crates/abb-engine`) owns the product's rules and the working session; the
Tauri app is one host for it. Views dispatch semantic intent, and typed
interfaces cross seams. A native UI would replace only the host tier.

## Product Spine

Audiobook Boss turns messy audiobook inputs plus user intent into organized,
tagged, chapterized audiobook files. AAC/M4B is the default; title audio plans
can choose MP3 pass-through or Opus in M4A/MKA.

```text
Import -> Inspect -> Decide -> Preflight -> Process -> Verify
```

- **Import:** discover or materialize local audio, then validate and analyze it.
- **Inspect:** probe audio, chapters, metadata, cover art, and compatibility.
  Cover-art load vs write-prep lives in `docs/cover-art-processing.md`.
- **Decide:** stage metadata, naming, destination, per-book audio handling,
  encoder, and collision intent.
- **Preflight:** resolve paths, capabilities, collisions, and the execution plan.
- **Process:** accept work, run jobs, publish progress, and settle cancellation
  or failure.
- **Verify:** report backend terminal truth and leave trustworthy artifacts on
  disk.

## Control Loop

Working session and settings (titles, selection, metadata edits, lookup, Save,
audio choices, output naming and estimates, submission, preferences):

```text
User intent
  -> Solid view
  -> src/app adapter owner               wording + local echo of typed text
  -> engineLink                          numbered intent
  -> tauriClient -> host command         run in the order sent (intent order)
  -> Engine session / settings runtime   rule, transition, file/network work
  -> reply + session/settings events     snapshot parts with revisions
  -> engineLink keeps newest parts -> Solid view renders
```

A submission continues inside the engine:

```text
Submit / Preview intent
  -> Engine session                      build the export from the session, hold its sources
  -> preflight plan                      paths, collisions, signature; ReviewRequired waits for a choice
  -> session preview | WorkRuntime      same progress reducer, distinct retained history
  -> Audio / Metadata / Output owners    owned side effects
  -> snapshots + artifact readback
  -> Solid view renders terminal truth
```

Remote Source materializes provider-owned titles into ABB-owned staged local
files, then the engine imports them into the working session like any other
files.

## Owners

| Owner | Path |
| --- | --- |
| App Runtime | `src/app/runtime` |
| Engine link | `src/app/engineLink` |
| Frontend owners | `src/app/<owner>` |
| Solid views | `src/ui/<owner>` |
| UI Foundation | `src/ui/foundation` |
| Tauri runtime boundary | `src/lib/tauri` |
| Tauri host | `src-tauri` (commands registered in `src-tauri/src/ipc_contract.rs`) |
| Working session | `crates/abb-engine/src/session` |
| Processing | `crates/abb-engine/src/processing` |
| WorkRuntime | `crates/abb-engine/src/work_runtime` |
| Active-work power | `crates/abb-engine/src/power.rs` |
| Audio Engine | `crates/abb-engine/src/audio` |
| Metadata Outcome | `crates/abb-engine/src/metadata` |
| Output Artifact | `crates/abb-engine/src/output_artifact` |
| App Settings | `crates/abb-engine/src/app_settings`, `src/app/appSettings` |
| Remote Source | `crates/abb-engine/src/remote_source`, `src/app/remoteSource` |
| Core crates | `crates/abb-*-core` (tier rules: `crates/AGENTS.md`) |

Each owner's nearest `AGENTS.md` states what it owns and its allowed import and
export surface.
