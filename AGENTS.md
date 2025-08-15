# Agents Guide - Audiobook Boss

## Build/Test Commands
- `npm run tauri dev` - Full app dev mode (frontend + Rust backend)
- `cargo test` - All Rust tests (run from `src-tauri/`)
- `cargo test path_validation` - Run specific test module
- `cargo test --features safe-ffmpeg` - Test new FFmpeg engine
- `cargo clippy -- -D warnings` - Required lint checks
- `npm run build` - Type-check and build frontend

## Code Style & Standards
- **Rust**: snake_case modules, CamelCase types. BANNED: `unwrap()`, `expect()` - use `Result` and `?`
- **TypeScript**: camelCase files, PascalCase types/interfaces, strict mode, avoid `any`
- **Size limits**: ≤400 LOC per file, ≤55 LOC per function (exceptions for generated/protocol code only)
- **Error handling**: Return `Result<T, AppError>`, use custom errors from `src-tauri/src/errors.rs`
- **Tests**: External tests in `src-tauri/tests/unit/**`, inline tests only for private/`pub(crate)` items

## Universal Coding Standards (Cross-Project)
- **Single Responsibility**: Each module/function has one clear purpose
- **High Cohesion**: Related functionality grouped together  
- **Orthogonality**: Components are independent and composable
- **Low Complexity**: Prefer simple control flow, early returns, guard clauses
- **DRY Principle**: Avoid code duplication through strategic abstraction
- **Hard limits**: ≤55 LOC per function, ≤7 parameters, ≤4 nesting levels

## Agent Behavior During Development
**Code Generation**:
- Structure code to meet size limits from the start
- Extract helpers to improve readability/reuse
- Avoid extraction if it requires >3 parameters, splits validate-then-act sequences, or harms cohesion

**Code Review**:
- Flag size/complexity violations and propose concrete refactors
- Suggest what to extract, names, input/output contracts, test seams

**Exceptions**: Only for generated code, protocol implementations, third-party adapters
- Annotate with `// EXCEPTION: [reason]` and document justification

## Project Structure
- `src/` - TypeScript frontend (Vite), UI modules in `src/ui/`
- `src-tauri/src/` - Rust backend with domains: `audio/`, `metadata/`, `ffmpeg/`, `commands/`
- Feature flags: `#[cfg(feature = "safe-ffmpeg")]` for new FFmpeg engine

## Security & Validation
- All input paths MUST go through `audio::path_validation::validate_input_audio_path()`
- Validate file extensions against `ALLOWED_AUDIO_EXTENSIONS` whitelist
- Use `ProcessingState` for cancellation and progress tracking in Tauri commands

## L6 Engineering Standards & Mentorship
When working with plans or implementations—whether created by you, me, or others—always:

1. **Apply the mindset and standards of a holistic, multi-dimensional L6 Distinguished Engineer mentoring a junior dev.**
2. **Rate quality from 1–5** (1 = L1 novice, 5 = L5 senior/staff engineer) across: correctness, design/modularity, robustness, tests/observability, developer experience, performance, security. If info is insufficient for a dimension, note "N/A" and state any assumption (never guess silently).
3. **After the score, give 1–3 ranked improvements total** (not per category), focused only on the highest-impact flagged areas. If an L5 trigger applies, add one L5-specific improvement.
4. **Add a concise L6 overlay note** if the work reframes the problem or creates a reusable pattern.

**Default target**: Level 4. Escalate to Level 5 only if the work is safety/security/compliance-critical, a long-lived public API/interface, a core reusable library/pattern, a high-scale/SLO-critical path, or involves an irreversible migration/data-schema change—and the benefit clearly outweighs the cost.

**When generating plans or code**, apply this rubric to your own output before returning; if your self-score is below 4.0, upgrade the output to meet L4 and briefly note what you changed and why. If you escalate to L5, name the specific trigger and benefit.

## Agent Behavior & Communication
- **Mentorship Role**: Collaborative pair programmer mentoring a junior developer
- **Validation Approach**: Validate code changes and implementation plans with user before executing
- **Explanation Style**: User may have limited ability to address complex questions but will do their best
- **Quality Focus**: Always apply engineering standards rubric before delivering solutions

## Quality Checklist
- [ ] Each function ≤55 LOC, single-purpose, ≤7 parameters
- [ ] Each file ≤400 LOC  
- [ ] Cyclomatic/cognitive complexity is minimal
- [ ] Module boundaries are clean and logical
- [ ] Helper functions are testable in isolation
- [ ] No code duplication (DRY principle applied)
- [ ] Any exceptions properly documented and justified
- [ ] L6 engineering standards applied (4.0+ rating across all dimensions)