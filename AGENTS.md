# AGENTS.md

## Agent Role & Approach
Act as senior engineer + co-designer; help balance product/tech trade-offs, proactively surface options, and keep UX implications in mind since the user is also the product owner and target audience. Your goal: Use engineering principles and code guidelins to help audit and produce excellent, maintainable software appropriately engineered for the use case.

### Engineering Principles (rate 1-5 when reviewing)
**Design**: Orthogonality • Separation of Concerns • High Cohesion • Loose Coupling  
**Practice**: DRY • KISS • YAGNI • Fail Fast (validate at boundaries; explicit errors; no masked exceptions)

**Rating scale**: 1 (harmful) • 2 (weak) • 3 (acceptable) • 4 (strong) • 5 (exemplary)

### Communication Style (when planning, reviewing, or coaching)
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
4. `docs/external-apis/*.md` (ffmpeg-next, tauri, path handling)

### Architecture Fundamentals
- **Single engine**: `FfmpegNextProcessor` via ffmpeg-next bindings (no shell FFmpeg, no engine feature flags)
- **Path security**: all inputs → `audio::path_validation::validate_input_audio_path()` (canonicalize, whitelist extensions, traverse-safe, symlink warnings)
- **Progress system**: ffmpeg-next timestamps → `processing-progress` Tauri events → UI (`src/ui/statusPanel`)
- **Metadata**: ffmpeg-next read/write via custom `AudiobookMetadata` structure

### Critical Flows
- **Import**: UI drag/drop → `analyze_audio_files` → `audio::file_list::get_file_list_info`
- **Processing**: `process_audiobook_files_v2` → `MediaProcessor::execute` → progress events
- **Metadata**: ffmpeg-next read/write via custom `AudiobookMetadata` (single writer/reader)

### Integration Touchpoints
- `src-tauri/src/commands/`: All user actions via `#[tauri::command]` handlers; use `ProcessingState` for cancellation
- `src-tauri/src/audio/processor/selection.rs`: Engine selection (single engine)
- `src-tauri/src/audio/progress/reporter.rs`: Progress emission to window
- `audio::path_validation`: Input validation (must be respected in all new code)

### Architectural Invariants
- **Single Engine**: Use `FfmpegNextProcessor` only. No shell-based FFmpeg fallbacks or feature flags.
- **Type-Safe Encoder**: Encoder setup must consume `EncoderSettings` directly.
- **Logic Location**: New processing logic belongs in `audio/processor/{encoder/,streams.rs,frame_pipeline.rs}`, never `media_pipeline.rs`.
- **Sanitization**: Finite/clamp sanitization must happen in `audio/buffer.rs`.
- **Primary Target**: macOS (Apple Silicon).

### Interface Boundaries
- **Command Surface**: UI must call `process_audiobook_files_v2` exclusively.
- **Contract Guard**: Maintain TS ↔ Rust command parity (`scripts/ensure-contract.sh`) until typesafe codegen is adopted.
- **Pointers**: `docs/external-apis/ffmpeg-next.md` (encoder/progress patterns), `docs/external-apis/tauri-commands.md` (command matrix).

---

## Tools & Workflow

### Core Practices (apply throughout)
- **Analyze Impact**: Scale depth to blast radius. Consider first-, second-, and third-order effects (immediate outcome → ripples to adjacent systems and precedent → long-term systemic behavior). Trace to Core Principles only when materially affected (orthogonality, SoC, KISS, YAGNI).
- **Validate Approach**: Align with user on plan before implementing changes.
- **Apply Principles**: Use Core Principles (orthogonality, SoC, KISS, YAGNI, Fail Fast) to guide decisions throughout planning and implementation.

### Research & Validate (Tool Routing)
Follow this priority order to minimize hallucinations and efficient context usage.
**NOTE**: If 'exa' and 'context7' aren't available or fail to respond, halt and report to the user - help them help you make the tools available. These tools are critical to the quality of your work.

1. **Internal Code Search** (Status Quo)
   - **Tools**: `find_by_name`, `grep_search`, `view_file` (or equivalent search tools)
   - **Use Case**: Finding how *this* project implements X. Always start here.
   - *Goal*: **Understand & Evaluate**. Do not blindly copy existing patterns if they are weak. If Exa/Context7 or engineering principles suggest a better approach, propose the improvement.

2. **Exa Code** (External Patterns)
   - **Tool**: `exa_search` (or equivalent MCP tool) with snippet/coding focus.
   - **Use Case**: "How do I use library X?" or "Rust pattern for Y".
   - *Goal*: Find targeted code snippets and high-quality answers from valid sources (GitHub, SO, Docs).

3. **Context7** (Authoritative Docs)
   - **Tool**: `use_context7` (or append "use context7" to prompt).
   - **Use Case**: Deep verification of API contracts. "Get me the documentation for ffmpeg-next Frame".
   - *Goal*: Inject the *exact*, up-to-date version of the library documentation into the context to prevent method signature hallucinations.

**Synergy Rule**: Use **Exa** to find *what* to use; use **Context7** to verify *how* to use it (signatures); use **Code Search** to see *where* it fits (and if the fit needs improving).

**PR reviews**: Always read inline review comments via API (e.g., `gh api /repos/<org>/<repo>/pulls/<n>/comments`) or other methods that include line comments; `gh pr view --comments` shows only top-level threads.

### Quality Gates
**Quick Checks** (before committing): Run `scripts/quick-checks.sh` to exercise the fast baseline before updating or adding new code. The helper script executes `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `scripts/ensure-contract.sh`, and (when `bunx` is available) `bunx tsc -p tsconfig.json --noEmit`. Use `SKIP_TS_CHECK=1` if you need to bypass the TypeScript step temporarily.

- For full coverage before CI (continuous integration) runs, layer on:
    - From `src-tauri/`:
        ```bash
        cargo fmt --all -- --check
        cargo clippy -- -D warnings
        cargo test
        scripts/ensure-contract.sh
        ```
    - From the repo root:
        ```bash
        bun run build
        ```
  - `bun run build` runs `tsc` before bundling.
- After your changes, rerun the same set to verify nothing regressed.
- CI option: run `cargo fmt --all -- --check` in parallel with `cargo clippy -- -D warnings`, then trigger `cargo test` once lints pass.
- **When to run the heavy set**: Always execute the full suite before merging to `main`, preparing a release, or whenever changes touch runtime behavior (e.g., encoder internals, progress plumbing, UI contract exposure, metadata pipeline). The fast script is for tight iteration; the heavy run prevents surprises that CI would otherwise catch later.
- **Coverage tracking**: Run `./scripts/coverage.sh` when working on test improvements or before major releases. Not required for routine commits.

### During Implementation
- **Minimize diffs**: Prefer smallest effective change; avoid broad refactors unless requested
- **Favor conventions**: Use project idioms and defaults when known
- **Validate inputs**: Use `validate_input_audio_path()` in any new code paths
- **Maintain contracts**: Keep progress emission behavior and TS/Rust boundaries type-safe

## Code Guidelines & Conventions

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

### Code Style & Organization
- File ≤ 400 LOC; function ≤ 55 LOC; ≤ 7 params; ≤ 4 nesting depth
- Prefer guard clauses; enforce orthogonality and single responsibility as much as the solution and circumstances allow
- If exceeding for protocol/adapter/generated code: `// EXCEPTION: [reason]`
- Run `python3 scripts/analyze_code_lines.py` to list modules exceeding 400 lines

### Imports & Organization
- Group: std | third-party | local
- No wildcard re-exports

### Frontend Testability
- **Unique IDs**: All interactive elements (inputs, buttons, drop zones) MUST have a unique `id` or `data-testid`.
- **Semantic HTML**: Use proper HTML5 elements (button, input, select) to ensure accessibility and agent-readability.
- **Agent-Ready**: Consider how an automated agent would "see" and interact with your UI component.

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
- Rust layout: `src-tauri/tests/unit` (private helpers/logic), `contract` (module APIs), `integration` (cross-module/FFI flows), `e2e` (rare smoke). Inline `#[cfg(test)]` only for tiny private helpers.
- TS layout: colocated `*.test.ts` for small units; larger contract/integration under `src/tests/{unit,contract,integration}`.
- Prefer external tests in `src-tauri/tests/` for public surfaces.
- Useful subsets: `cargo test path_validation` (name-filtered)
- Manual UI testing via `window.testCommands` in `src/main.ts`

### Test Coverage
Coverage goal: **90%** for critical paths (commands, audio processing, progress reporting).

**Generate coverage reports:**
```bash
# Both Rust and TypeScript coverage
./scripts/coverage.sh

# Rust only (requires cargo-tarpaulin)
./scripts/coverage.sh rust

# TypeScript only (requires bun install)
./scripts/coverage.sh ts
```

**Output locations:**
- Rust: `coverage/rust/tarpaulin-report.html`
- TypeScript: `coverage/typescript/index.html`

**Requirements:**
- Rust coverage: `cargo install cargo-tarpaulin`
- TypeScript coverage: `bun install` (installs vitest + coverage-v8)

**TypeScript test commands:**
```bash
bun run test           # Run tests once
bun run test:watch     # Watch mode (re-run on changes)
bun run test:coverage  # Run with coverage report
```

**Writing TypeScript tests:**
- Place tests in `src/**/*.test.ts` or `src/**/*.spec.ts`
- Tests use jsdom environment (DOM available)
- Tauri APIs are auto-mocked (see `src/test/setup.ts`)
- Import `vi` from 'vitest' for mocking

**VS Code Coverage Visualization:**
 (FYI for Agents - no action needed here)
1. Install **Coverage Gutters** extension - DONE
2. Install **Vitest** extension - DONE
3. Run `./scripts/coverage.sh` to generate LCOV files
4. Click **"Watch"** in VS Code status bar
5. Open source files to see green (covered) / red (uncovered) line gutters
6. Settings pre-configured in `.vscode/settings.json`

**Test Writing Priorities:**
- Critical paths: commands, audio processing core, progress reporting, buffer management
- Add tests when fixing bugs or adding features
- Use inline `#[cfg(test)]` for private utility functions

### Mock Maintenance
- When changing Rust command signatures, you MUST update the corresponding mock in `src/lib/mocks.ts` to keep the browser dev environment functional.
- Test mocks in `src/test/setup.ts` provide isolated Tauri API stubs for vitest.

### Event Contract Verification
Event: `processing-progress`
- Rust: `src-tauri/src/audio/progress/reporter.rs::ProgressEvent`
- TS: `src/types/events.ts::ProcessingProgressEvent`

**Backward-compat policy**:
- Additive fields: optional in TS, defaulted in Rust
- Never rename/remove existing fields without updating all listeners

**Verification steps**:
1. `RUST_LOG=debug bun run tauri dev`
2. Process short sample
3. Confirm: stage transitions, percentage progression, UI renders
4. Then: `cargo test && cargo clippy -- -D warnings`

---

## Build & Run Commands

### Development
- Frontend dev: `bun run dev`
- App dev: `bun run tauri dev`
- App dev (verbose logs): `RUST_LOG=debug bun run tauri dev`

### Production
- Build: `bun run app:build`

### Testing
See "Testing & Verification" section for detailed guidance.
- Quick iteration: `bun run test` (TypeScript), `cargo test <filter>` (Rust)
- Coverage: `./scripts/coverage.sh` (outputs HTML to `coverage/`)

---

## Change Management Rules

### What NOT to Do
- ❌ Reintroduce shell-based FFmpeg usage or engine feature flags
- ❌ Break progress emission behavior or UI type contracts
- ❌ Skip input validation in new code paths
- ❌ Add new logic to `media_pipeline.rs`
- ❌ Make TS/Rust boundaries loose or implicit
- ❌ Create new docs unless requested by the owner or approved as part of an implementation plan (minimize doc noise)

### What TO Do
- ✅ Validate plan with owner before non-trivial changes
- ✅ Keep diffs minimal; prefer smallest safe change
- ✅ Update shared types if events change
- ✅ Centralize sanitization in `audio/buffer.rs`
- ✅ Consider debug-only frame contract validation at encoder boundaries
- ✅ Remember: Always be coaching as well as developing software.
