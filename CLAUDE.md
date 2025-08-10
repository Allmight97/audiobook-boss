---
alwaysApply: true
---
## Project Structure & Module Organization
- `src/`: Frontend (Vite + TypeScript). UI modules in `src/ui/`, shared types in `src/types/`, assets in `src/assets/`.
- `src-tauri/`: Rust backend for Tauri. Code in `src-tauri/src/` with domains like `audio/`, `metadata/`, `ffmpeg/`, `commands/`.
- `docs/`, `media/`: Documentation and media assets. `dist/`: Vite build output.
- `src-tauri/binaries/`: FFmpeg bundling helpers. `scripts/`: project scripts.

## Build, Test, and Development Commands
- `npm run tauri dev`: Start the full desktop app (frontend + Rust).
- `npm run dev`: Frontend dev server only (port 1420).
- `npm run build`: Type-check and build frontend to `dist/`.
- `npm run build-macos` / `npm run package-macos`: Prepare FFmpeg and build/package macOS app.
- `npm run setup-ffmpeg`: Download/bundle FFmpeg for macOS.
- Rust tests (from `src-tauri/`): `cargo test` (optionally `cargo clippy` for lints).
- **Clippy verification**: Both `cargo clippy -- -D warnings` and `cargo clippy --features safe-ffmpeg -- -D warnings` must pass - IF THEY DON'T report to the user after a single round of root cause 5-whys analysis.

## Commit & Pull Request Guidelines
- Commits: concise, imperative, and scoped (e.g., "Refactor FFmpeg path handling"). Emojis are fine; keep subject ≤ 72 chars.
- PRs: include a clear summary, linked issues, steps to test, and screenshots/GIFs for UI changes.
- Checks: `cargo test` passes, `npm run build` succeeds, and app runs via `npm run tauri dev`. Avoid committing large binaries—use `src-tauri/binaries/` scripts.

## Security & Configuration Tips
- Logging: set `RUST_LOG` (e.g., `RUST_LOG=info npm run tauri dev`).
- FFmpeg: prefer `npm run setup-ffmpeg`; the bundled binary is referenced in `tauri.conf.json` (`bundle.externalBin`).

## Testing Guidelines
- Rust: place external tests under `src-tauri/tests/unit/**` by domain (e.g., `audio/`, `commands/`, `ffmpeg/`, `metadata/`). Use inline tests inside modules only when required to test non-`pub` internals (including `pub(crate)`, `pub(super)`, `pub(in ...)`). Run with `cargo test` in `src-tauri/`.
- Frontend: no formal test runner yet; use `window.testCommands` in `src/main.ts` for manual validation paths.
- Coverage focus: `audio` pipeline, Tauri command handlers, and metadata read/write; name tests by behavior and module.
- Tests: prefer external tests in `src-tauri/tests/unit/**` for public APIs/behavior; use inline tests only when needed to cover private/internal items that aren’t accessible externally.
  - Use inline tests only when testing items that are:
  - Private (`fn` without `pub`)
  - Module-scoped (`pub(crate)`, `pub(super)`, `pub(in path)`) or
  - Cannot be meaningfully tested through public APIs

## Coding Style & Naming Conventions
- TypeScript: strict mode; prefer explicit types, avoid `any`. Module files follow kebab-case (e.g., `file-list.ts`), types/interfaces PascalCase.
- Rust: follow Rust conventions (snake_case modules, CamelCase types). Lints: `#![deny(clippy::unwrap_used)]` and `#![warn(clippy::too_many_lines)]` are enforced—avoid `unwrap`; use `Result` and `?`.
- **Strategic allows**: Use `#[cfg_attr(not(any(test, feature = "safe-ffmpeg")), allow(unused_imports))]` for feature-gated infrastructure vs broad `#[allow()]`.
- Formatting: use default rustfmt; for TS, rely on `tsc` + Vite. Keep functions small and focused.
- Visibility: keep private/internal items non-`pub` unless required by cross-module use.

## Code Size & Modularity Standard
Principles:
- Uphold Single Responsibility, high cohesion, and orthogonality (independent components).
- Prefer low cyclomatic/cognitive complexity via simple control flow and early returns.

Hard limits:
- Module/file: ≤ 400 lines of code (exclude comments/blank).
- Function/method: ≤ 60 lines of code (exclude comments/blank).

Agent behavior:
- During generation: structure code to meet limits; extract helpers without harming cohesion.
  - Avoid extracting helpers when it would:
    - Require passing >3 parameters between functions
    - Split logically atomic operations (e.g., validate-then-act patterns)
    - Create circular dependencies between modules
- During review: flag any violation and propose concrete refactors (what to extract, names, inputs/outputs, test seams).
- Only exceed limits for justified generated/protocol glue; annotate with a comment with `// EXCEPTION:` and create a refactor ticket
  - Exceeding limits allowed only for:
    - Generated code (proc macros, build scripts)
    - Protocol implementations (Tauri command handlers, serialization)
    - Third-party interface adapters (FFmpeg bindings, external APIs)

Checklist before returning code:
- [ ] Each function ≤ 60 LOC and single-purpose
- [ ] File ≤ 400 LOC
- [ ] Complexity is low; boundaries clean; helpers testable
- [ ] Any exception annotated + ticketed