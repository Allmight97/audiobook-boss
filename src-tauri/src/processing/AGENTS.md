## Public API Strip
- Import from `crate::processing::plan`, not private helpers.
- Functions: `resolve_preflight_plan`, `prepare_execution_plan`.
- Types: `ResolvedProcessingPlan`, `PlannedProcessingJob`.

## Private Cluster
- Files: `../processing.rs`, `plan.rs`, `run.rs`, `terminal_outcomes.rs`, `context/`, `job_registry/`, `progress/`, `preview_config.rs`, `session.rs`, `contract_tests.rs`.
- The cluster owns preflight planning, execution-plan preparation, runner orchestration, processing context/session state, job lifecycle, progress event types, terminal result normalization, and their behavior tests.

## Allowed Agent Edits Without Escalation
- Change planner or runner internals when `cargo test contract_tests` and `scripts/check-public-api-strips.sh` stay green.
- Keep preflight side-effect-free; execution may create output dirs only after review enforcement.
- Keep runner responsibilities to encoder/toolchain validation, events, job registration, scheduler dispatch, audio execution requests through `crate::audio`, and terminal normalization.

## Breaking-Change Triggers
- Adding, removing, or renaming any Public API Strip symbol.
- Changing preflight signature behavior, collision-review enforcement, metadata projection, path validation, or parent-dir side effects.
- Moving artifact truth, metadata intent semantics, or status terminal truth out of their owning boundaries.
