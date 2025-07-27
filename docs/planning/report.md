# Code Quality Audit Report

_Last updated: 2025-07-27_

## 0. Executive Summary
This document records the issues discovered during our July-2025 review of the **audiobook-boss** Rust backend and provides a prioritised remediation roadmap.  It complements – and must be read together with – `implementation_technical_plan.md`, which details *how* we will address each item.

## 1. Scope & Methodology
* **Scope** – Source files under `src-tauri/src` and the existing test suite (≈130 tests).
* **Tools** – `tokei`, `cargo clippy`, `cargo llvm-lines`, selective grep searches.
* **Criteria** – Function length, module size, unsafe patterns (`unwrap()`), duplication, and adherence to Phases 1-4 objectives.

## 2. Key Findings
| ID   | Severity     | Finding                                                                              | Evidence                                                                                                               |
| ---- | ------------ | ------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------- |
| F-01 | **Critical** | Five production modules exceed 400 LOC, breaking SRP and complicating review.        | `processor.rs` 1455 LOC, `cleanup.rs` 946 LOC, `context.rs` 804 LOC, `progress.rs` 485 LOC, `commands/mod.rs` 438 LOC. |
| F-02 | **Critical** | 21 functions still exceed the 30-line guideline and several exceed 100 lines.        | `processor.rs`, `commands/mod.rs`.                                                                                     |
| F-03 | **High**     | 40+ `unwrap()`/`expect()` calls remain in production paths – risk of runtime panics. | grep results, esp. `context.rs` and `processor.rs`.                                                                    |
| F-04 | **High**     | Duplicate FFmpeg time-parsing logic in `progress.rs` and `processor.rs`.             | manual diff.                                                                                                           |
| F-05 | **Medium**   | `lock().unwrap()` on shared mutexes may introduce contention.                        | `context.rs` state locks.                                                                                              |
| F-06 | **Medium**   | Tests embedded in production files inflate LOC counts and blur boundaries.           | `progress.rs`, `processor.rs`.                                                                                         |
| F-07 | **Nice**     | Metrics scaffold (`audio/metrics.rs`) exists but not integrated.                     | untracked file.                                                                                                        |

## 3. Impact Analysis
• **Stability** – F-03 can crash the app on malformed input.<br>
• **Maintainability** – F-01/02 create high cognitive load; slow code review.<br>
• **Performance** – F-05 may block UI under heavy load.<br>
• **Extensibility** – Large public surfaces hinder safe feature addition.

## 4. Remediation Roadmap (Phased)
The table links each finding to one or more phases of the implementation plan.

| Phase   | Objective                         | Mapped Findings        | Outcome Metric                                          |
| ------- | --------------------------------- | ---------------------- | ------------------------------------------------------- |
| Phase 0 | Bootstrap CI & guard scripts      | All                    | CI fails if new code violates length limits.            |
| Phase 1 | Split `processor.rs`              | F-01, F-02, F-03, F-04 | `processor.rs` ≤ 300 LOC; no duplicate parsing.         |
| Phase 2 | Split `cleanup.rs` & `context.rs` | F-01, F-02, F-03, F-05 | Each new file ≤ 300 LOC; no panics.                     |
| Phase 3 | Decompose `progress.rs`           | F-01, F-02, F-04, F-06 | Separate parser/emitter; tests moved.                   |
| Phase 4 | Hardening & Unwrap Removal        | F-03, F-05             | Zero `unwrap()` in production; mutex strategy reviewed. |
| Phase 5 | Docs & Metrics integration        | F-06, F-07             | Metrics recorded in logs; architecture doc updated.     |

The phases are deliberately **front-loaded with critical issues** (size, panics) before “nice-to-have” tasks (metrics, docs).

## 5. Next Steps
1. Adopt `implementation_technical_plan.md` unchanged *except* for aligning its **Stage** labels with the **Phase** labels above (already done in v2).  
2. Create tracking issues/PRs per task list so work can be distributed to human devs or automated agents.  
3. Set CI to block merges until Phase 0 tasks pass.

## 6. Appendix A – Raw Metrics
_(kept brief – consult `report.html` artefact in CI for full numbers)_
```
1455 src-tauri/src/audio/processor.rs
 946 src-tauri/src/audio/cleanup.rs
 804 src-tauri/src/audio/context.rs
 485 src-tauri/src/audio/progress.rs
 438 src-tauri/src/commands/mod.rs
``` 