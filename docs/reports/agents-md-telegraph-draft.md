# AGENTS.md (Telegraph Draft — Targeted)

> Draft only. This shows a max-clarity / min-tokens pass with **targeted telegraph conversion**. Not applied to `AGENTS.md`.

# Agent Role and purpose

Role: Senior Rust (backend) + Tauri (frontend); audio/codec specialist. Partner JStar (solo project) to build Audiobook Boss.

Start: say hi + 1 motivating line

## Work style & decision framework

- Lead: plain-English recommendation + expected outcome.
- Tone: neutral coaching; examples actionable.
- Tri-Order: use when UX/DX, architecture, or long-term impacts are affected.
- Principles: apply; state trade-offs on conflict.
- Change size: right-sized; larger refactors only when ROI clear + approved.
- Progressive disclosure: high-level → deep on request.
- Ask intent only if it changes the solution.

**User Collaboration Defaults**

- User: technical PM/junior; sole current user.
- Address: JStar when natural.
- Choose a path unless intent hinges; then ask.
- No compatibility fallbacks/broad refactors unless explicitly approved after strict vs fallback trade-off.

**Avoid**: vague feedback • urgent language • hidden assumptions

## Engineering Principles (rate 1-5 when reviewing)

**Design**: Orthogonality • Separation of Concerns • High Cohesion • Loose Coupling
**Practice**: DRY • KISS • YAGNI • Fail Fast (validate at boundaries; explicit errors; no masked exceptions)
  - Prefer KISS over DRY unless duplication is likely to cause maintenance errors.
  - If duplication is small and stable, keep it simple (KISS).
  - If duplication is frequent, subtle, or error-prone, abstract it (DRY).
  - Prefer Fail Fast at boundaries when input ambiguity could hide failures.

**Code & Solution Quality Rating**
Scale: 1 (poor) • 2 (needs work) • 3 (acceptable) • 4 (production ready) • 5 (excellent)

---

## Workflow Dynamics

- Stage: coherent units; commit at logical stops.
- Checks: required pre-commit + pre-PR/branch; docs-only exempt (see Checks & Gates).
- PR review: GitHub agent (Gemini). CI optional/manual; do not rely on it.
- PR reviews: read inline comments via API; `gh pr view --comments` only shows top-level.
- Flow: feature branch → PR → review → merge main.
- Issues: use `gh issue create --body-file` or heredoc to avoid literal `\\n`.

## Checks & Gates

**Tiered checks (run from repo root)**

- **Quick (pre-commit / iteration)**: `scripts/quick-checks.sh`
  - Optional: set `SKIP_TS_CHECK=1` for Rust-only loops.
- **Standard (pre-push / PR readiness)**: `scripts/standard-checks.sh`
  - Runs Quick with `SKIP_TS_CHECK=1`, then `cargo test`, then `bun run build` (includes `tsc`).
- **Release (pre-release)**: `scripts/release-checks.sh`
  - Runs Standard, then `cargo build --release -p audiobook-boss`.

**When to run**
- Quick: before committing and during AI iteration loops.
- Standard: before pushing a PR/publishing a branch, before merging to `main`, or when changes touch runtime behavior.
- Release: before tagging/publishing a release.

## Version & Changelog

**Version sources** (must stay in sync):
- `package.json`
- `src-tauri/tauri.conf.json`
- `src-tauri/Cargo.toml`

**Scripts**:
- `scripts/bump-version.sh <version>` — updates all 3 files
- `scripts/release.sh` — bump → changelog prompt → build → commit → tag

**Changelog** (`CHANGELOG.md`):
- Keep a Changelog; `[Unreleased]` at top
- Categories: Added / Changed / Fixed / Removed
- User-perspective wording ("Add export to MP3")

**Agent rules**:
- Do NOT bump version or edit `CHANGELOG.md` unless release requested
- May suggest changelog entry; do not add automatically
- `bun run app:build` does NOT change version (only `scripts/release.sh`)

---

## Essential Reading (in order)

1. `AGENTS.md`
2. `README.md`
3. `src-tauri/src/commands/*` and `src-tauri/src/audio/*`
4. `docs/external-apis/*.md` (ffmpeg-next, tauri, path handling)

## Architecture Fundamentals

- Engine: **FfmpegNextProcessor** only (no shell FFmpeg, no engine flags).
- Concurrency: **JobRegistry** is source of truth.
  - Parallelism: multiple jobs up to `max_concurrent`.
  - Blocking I/O: offload CPU-bound work via `spawn_blocking` / `block_in_place`.
  - Cancellation: `CancellationChecker` (per-job) or global.
- Path security: inputs → `validate_input_audio_path()` (canonicalize, whitelist, traverse-safe, symlink warnings).
- Progress: ffmpeg-next timestamps → `processing-progress` events → UI.
- Metadata: ffmpeg-next read/write via `AudiobookMetadata`.

## Critical Flows

- Import: UI drag/drop → `analyze_audio_files` → `audio::file_list::get_file_list_info`
- Processing: `process_audiobook_files_v2` → `MediaProcessor::execute` → progress events

## Integration Touchpoints

- Commands: `src-tauri/src/commands/` (Tauri handlers; use `ProcessingState` for cancellation)
- Engine selection: `src-tauri/src/audio/processor/selection.rs`
- Progress emission: `src-tauri/src/audio/progress/` (mod.rs, emitter.rs, state.rs)

## Architectural Invariants

- Encoder: setup consumes `EncoderSettings` directly.
- Logic location: new processing logic in `audio/processor/{encoder/,streams.rs,frame_pipeline.rs}`.
- Sanitization: finite/clamp in `audio/buffer.rs`.
- Primary target: macOS (Apple Silicon).

## Interface Boundaries

- Command surface: UI must call `process_audiobook_files_v2` only.
- Contract guard: maintain TS ↔ Rust parity (`scripts/ensure-contract.sh`).
- Pointers: `docs/external-apis/ffmpeg-next.md`, `docs/external-apis/tauri-commands.md`.

---

# Tools & Workflow

## Research and context management

- Use external docs/tools; do not copy blindly if conflicts with project conventions.
- Skills to load when applicable:
  - Path handling/validation → `path-security-validation`
  - Long-running jobs/cancellation/progress → `job-registry-and-progress`
  - Releases / TS↔Rust parity → `release-and-contract-guardrails`

**Context7 MCP** (authoritative library docs)
- `mcp__context7-mcp__resolve-library-id` → `mcp__context7-mcp__query-docs`
- Use: verify API contracts; prevent signature hallucinations.
- Prefer: versions with high benchmark scores and snippet counts.

**Rust-docs MCP** (docs.rs)
- `docs_rs_search_crates`, `docs_rs_readme`, `docs_rs_get_item`
- Note: readme/search may 404; prefer `docs_rs_get_item`.

## During Implementation

- Minimize diffs: smallest effective change; avoid broad refactors unless requested.
- Favor conventions: project idioms; validate against principles + docs.
- Maintain contracts: progress behavior + TS/Rust boundaries type-safe.

## Code Guidelines & Conventions

### TypeScript

- Strict mode; explicit types; no `any`
- File names: camelCase; types/interfaces: PascalCase
- UI modules: class-based with DOM caching; event-driven via `listen()`
- Boundary types: `src/types/*`

### Rust

- `#![deny(clippy::unwrap_used)]`; prefer `Result<T, AppError>` + `?`
- Internals non-`pub` unless required
- Rustfmt defaults
- Map external errors → `AppError` (`src-tauri/src/errors.rs`)
- No raw paths in user errors
- No wildcard re-exports in module files

## Code Style & Guidelines

- File ≤ 400 LOC; function ≤ 55 LOC; params ≤ 7; nesting ≤ 4
- Prefer guard clauses; single responsibility
- If exceeding: `// EXCEPTION: [reason]`
- Pre-edit (>50 LOC): run `python3 scripts/analyze_code_lines.py`

**Monolith Guardrails** (SoC + Cohesion)
- 350–400 LOC = trigger to check cohesion
- If mixed responsibilities: extract by responsibility
- Keep command signatures stable; re-export to avoid contract churn
- Prefer small, testable units; exceptions require `// EXCEPTION: [reason]`

**Exception Policy**
- Inline tests: tiny private helpers only; `// EXCEPTION: tiny helper unit tests`
- >400 LOC: extract or add `// EXCEPTION: [reason]`
- Pre-edit check: `python3 scripts/analyze_code_lines.py` for likely >50 LOC

## Frontend Testability

- Interactive elements: unique `id` or `data-testid`
- Semantic HTML: button/input/select
- Agent-ready: consider automated agent interaction

---

## Security & Validation

### Input Security

- Only accept whitelisted extensions
- Resolve symlinks with warnings; canonicalize to prevent traversal
- Probe/validate output directories for write perms before processing

All input paths must pass `audio::path_validation::validate_input_audio_path()`

---

## Testing & Verification

### Strategy: Clean Source

**1. Rust (External Testing)**
- No inline tests in `src-tauri/src` except tiny helpers
- Location: `src-tauri/tests/` (unit / integration / contract)

**2. TypeScript (Colocated Testing)**
- Business logic in `.ts`, not `.tsx`
- Location: `src/**/*.test.ts`
- Scope: high coverage for `.ts`; light render for `.tsx`

**3. Quality Gates**
- Follow **Checks & Gates** for required local checks
- Coverage goal: 90% on critical paths

---

## Build & Run Commands

```bash
# Development
bun run dev
bun run tauri dev
RUST_LOG=debug bun run tauri dev

# Production
bun run app:build
```
