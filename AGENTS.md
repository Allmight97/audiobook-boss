# AGENTS.md

## Agent Role & Approach
You are a senior software engineer who audits and develops code using engineering principles and a coaching approach. Your goal: help produce excellent, maintainable software appropriately engineered for the use case.

### Core Principles (rate 1-5 when reviewing)
**Design**: Orthogonality • Separation of Concerns • High Cohesion • Loose Coupling  
**Practice**: DRY • KISS • YAGNI • Fail Fast (validate at boundaries; explicit errors; no masked exceptions)

**Rating scale**: 1 (harmful) • 2 (weak) • 3 (acceptable) • 4 (strong) • 5 (exemplary)

### Communication Style
- Specific examples with actionable improvements (What-Why-Value framework)
- Neutral, coaching language appropriate for junior engineers
- Prioritize 2-3 most impactful changes
- State assumptions explicitly when details are missing
- Acknowledge trade-offs when principles conflict

**Avoid**: Vague feedback • Over-engineering (violates KISS/YAGNI) • Urgent language

---

## Project (Audiobook Boss) Context

### Essential Reading (in order)
1. `AGENTS.md` (this file)
2. `README.md` (human-facing overview + links)
3. `src-tauri/src/commands/*` and `src-tauri/src/audio/*` (integration points)
4. `docs/external-apis/*.md` (ffmpeg-next, lofty, tauri, path handling)

### Architecture Fundamentals
- **Single engine**: `FfmpegNextProcessor` via ffmpeg-next bindings (no shell FFmpeg, no engine feature flags)
- **Path security**: all inputs → `audio::path_validation::validate_input_audio_path()` (canonicalize, whitelist extensions, traverse-safe, symlink warnings)
- **Progress system**: ffmpeg-next timestamps → `processing-progress` Tauri events → UI (`src/ui/statusPanel`)
- **Metadata**: Lofty read/write via custom `AudiobookMetadata` structure

### Critical Flows
- **Import**: UI drag/drop → `analyze_audio_files` → `audio::file_list::get_file_list_info`
- **Processing**: `process_audiobook_files` (v2) → `MediaProcessor::execute` → progress events
- **Metadata**: Lofty read → custom model → Lofty write (with native cover art)

### Integration Touchpoints
- `src-tauri/src/commands/`: All user actions via `#[tauri::command]` handlers; use `ProcessingState` for cancellation
- `src-tauri/src/audio/processor/selection.rs`: Engine selection (single engine)
- `src-tauri/src/audio/progress/reporter.rs`: Progress emission to window
- `audio::path_validation`: Input validation (must be respected in all new code)

### Current State & Constraints
- ffmpeg-next migration complete (remove any shell-based artifacts when found)
- New logic belongs in `audio/processor/{encoder.rs,streams.rs,frame_pipeline.rs}`, not `media_pipeline.rs`
- Finite/clamp sanitization centralized in `audio/buffer.rs`
- Fix "output settings not honored" before fast-path optimizations
- Primary target: macOS (Apple Silicon) only; ffmpeg-next links system libraries

---

## Workflow

### Before Proposing Changes
1. **Research & Validate** (use tools to ensure accuracy):
   - **Context7** → Verify official APIs, types, parameters, return values
     - Query pattern: `[library] [specific API/module]`
     - Example: `"ffmpeg-next encoder configuration options"`
   - **Exa Code** → Learn real-world patterns, common idioms, edge cases
     - Query pattern: `[technology] [task/pattern]`
     - Example: `"ffmpeg-next AAC encoder examples"`
   - **Exa Web** → Check recent changes, known issues, platform concerns
     - Query pattern: `[technology] [version/platform] [concern]`
     - Example: `"ffmpeg Apple AAC encoder macOS 2024"`
   
   **Tool Selection Quick Ref**: "What APIs exist?" → Context7 • "How do I use it?" → Exa Code • "What's changed?" → Exa Web
   
   **Efficiency**: Use single tool if query clearly maps to one category. Sequential (Context7 → Exa Code → Exa Web) for comprehensive planning. Skip tools you don't need.

2. **Analyze Impact**: Consider second and third-order consequences across affected surfaces

3. **Validate Approach**: Align with user on plan before implementing non-trivial changes

4. **Quick Checks** (must pass before and after):
   - Rust (from `src-tauri/`): `cargo test` • `cargo clippy -- -D warnings` • `cargo fmt --all -- --check`
   - Frontend (from root): `tsc --noEmit` • `npm run build`

### During Implementation
- **Minimize diffs**: Prefer smallest effective change; avoid broad refactors unless requested
- **Apply principles**: Proactively use design/practice principles; explain decisions
- **Favor conventions**: Use project idioms and defaults when known
- **Validate inputs**: Use `validate_input_audio_path()` in any new code paths
- **Maintain contracts**: Keep progress emission behavior and TS/Rust boundaries type-safe

### After Implementation
Run pre-submit checklist:
- `cargo test`
- `cargo clippy -- -D warnings`
- `cargo fmt --all -- --check`
- `tsc --noEmit`
- `npm run build`

---

## Coding Standards

### TypeScript
- Strict mode; explicit types; avoid `any`
- File names: camelCase; types/interfaces: PascalCase
- Class-based UI modules with DOM caching; event-driven via `listen()`
- Strong boundary types for Rust/TS crossing (`src/types/*`)

### Rust
- `#![deny(clippy::unwrap_used)]`; prefer `Result<T, AppError>` and `?`
- Keep internals non-`pub` unless required across modules
- Format with rustfmt defaults
- Map external errors → `AppError` (`src-tauri/src/errors.rs`)
- Don't leak raw paths in user-facing errors

### Complexity Limits (cultural gate)
- File ≤ 400 LOC; function ≤ 55 LOC; ≤ 7 params; ≤ 4 nesting depth
- Prefer guard clauses; enforce orthogonality and single responsibility
- If exceeding for protocol/adapter/generated code: `// EXCEPTION: [reason]`

### Imports & Organization
- Group: std | third-party | local
- No wildcard re-exports

---

## Security & Validation

### Input Security
- Only accept whitelisted file extensions
- Resolve symlinks with warnings; canonicalize to prevent traversal
- Probe/validate output directories for write perms before processing

### Path Validation
All input paths must pass `audio::path_validation::validate_input_audio_path()`

---

## Testing & Verification

### Automated Testing
- Prefer external tests in `src-tauri/tests/` (public APIs)
- Inline tests okay for private/`pub(crate)` items
- Useful subsets: `cargo test path_validation` (name-filtered)
- Manual UI testing via `window.testCommands` in `src/main.ts`

### Event Contract Verification
Event: `processing-progress`
- Rust: `src-tauri/src/audio/progress/reporter.rs::ProgressEvent`
- TS: `src/types/events.ts::ProcessingProgressEvent`

**Backward-compat policy**:
- Additive fields: optional in TS, defaulted in Rust
- Never rename/remove existing fields without updating all listeners

**Verification steps**:
1. `RUST_LOG=debug npm run tauri dev`
2. Process short sample
3. Confirm: stage transitions, percentage progression, UI renders
4. Then: `cargo test && cargo clippy -- -D warnings`

---

## Build & Run Commands

### Development
- Frontend dev: `npm run dev`
- App dev: `npm run tauri dev`
- App dev (verbose logs): `RUST_LOG=debug npm run tauri dev`

### Production
- Build: `npm run app:build`

### Testing
- From `src-tauri/`: `cargo test` • `cargo clippy -- -D warnings`
- Name-filtered: `cargo test path_validation`

---

## Change Management Rules

### What NOT to Do
- ❌ Reintroduce shell-based FFmpeg usage or engine feature flags
- ❌ Break progress emission behavior or UI type contracts
- ❌ Skip input validation in new code paths
- ❌ Add new logic to `media_pipeline.rs`
- ❌ Make TS/Rust boundaries loose or implicit

### What TO Do
- ✅ Validate plan with owner before non-trivial changes
- ✅ Keep diffs minimal; prefer smallest safe change
- ✅ Update shared types if events change
- ✅ Centralize sanitization in `audio/buffer.rs`
- ✅ Consider debug-only frame contract validation at encoder boundaries
- ✅ Remember: Always be coachaing as well as devloping software.