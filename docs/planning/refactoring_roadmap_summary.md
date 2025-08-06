# Audiobook Boss - Refactoring Roadmap

**Purpose**: Master plan for transforming audiobook-boss from a state of high technical debt to a production-ready application.

---

## The Three-Phase Plan

This roadmap breaks down the task of refactoring a 1,455-line monolith into three manageable, sequential plans.

**Problem Statement**: 
- `processor.rs` has 1,455 lines, making it difficult to maintain.
- Significant DRY violations are blocking feature development.

**Solution**: A three-phase approach with clear, sequential steps.

---

### **Phase A: Stabilization**
🔗 Detailed FFmpeg-next migration plan: `FFMPEG_MIGRATION_RECOMMENDATIONS.md`
📁 `docs/planning/plan_a_emergency_stabilization.md`

**Focus**: High-priority refactoring of blocking issues.

**Goals**:
- Reduce `processor.rs` from 1,455 lines to a more manageable size (target: <500 lines).
- Eliminate high-priority DRY violations in progress tracking.
- Complete initial migration to the `ffmpeg-next` crate (see `FFMPEG_MIGRATION_RECOMMENDATIONS.md`).
- Create extraction patterns for subsequent phases to follow.

**Rationale**: These issues block all audio processing feature development.

**Success Criteria**:
- ✅ `processor.rs` is a manageable size (<500 lines).
- ✅ Progress tracking has a single source of truth.
- ✅ All 130+ tests continue to pass.
- ✅ A foundation is ready for systematic refactoring.

---

### **Phase B: Systematic Module Splitting**
📁 `docs/planning/plan_b_systematic_module_splitting.md`

**Focus**: Refactoring remaining oversized modules.

**Goals**:
- Split `cleanup.rs` (946 lines) into 3 sub-modules.
- Split `context.rs` (804 lines) into 3 sub-modules.
- Split `progress.rs` (485 lines) into 2 sub-modules.
- Split `commands/mod.rs` (438 lines) into 3 sub-modules.

**Rationale**: With patterns from Phase A established, these modules are safer to split. They have fewer DRY violations and clearer boundaries.

**Success Criteria**:
- ✅ All modules are ≤400 lines (exceptions allowed for 'processor.rs' for now)
- ✅ The facade pattern is consistently applied.
- ✅ Moderate DRY violations are eliminated.
- ✅ The codebase is ready for feature development.

---

### **Phase C: Quality Enhancement**
📁 `docs/planning/plan_c_quality_enhancement.md`

**Focus**: Polish and developer experience improvements.

**Goals**:
- Eliminate remaining minor DRY violations.
- Standardize all functions to ≤30 lines.
- Establish consistent naming conventions.
- Create developer tooling and documentation.

**Rationale**: This is polish work that improves the developer experience and long-term maintainability.

**Success Criteria**:
- ✅ Production-ready code quality.
- ✅ Improved developer experience.
- ✅ Systems for long-term maintenance are in place.
- ✅ Knowledge transfer documentation is created.

---

## Safety & Validation

### Risk Management Approach
1. **Incremental changes**: one module at a time, testing after each change.
2. **Version control**: commit after each successful extraction for easy rollback.
3. **Validation gates**: tests and clippy must pass before proceeding.
4. **Proven patterns**: follow existing `ffmpeg/` and `metadata/` examples.

---

## Codebase State Comparison

### Initial State
```
- processor.rs: 1,455 lines
- cleanup.rs: 946 lines
- context.rs: 804 lines
- progress.rs: 485 lines
- commands/mod.rs: 438 lines
- Multiple functions 50-100+ lines
- High-priority DRY violations blocking changes
```

### After Phase A (Stabilized)
```
- processor.rs: <500 lines (manageable)
- Progress tracking: centralized utilities
- Audio processing features can be added more safely
- Foundation for systematic refactoring is in place
```

### After Phase B (Modularized)  
```
- All modules: ≤400 lines
- Facade pattern: consistently applied
- Module boundaries: clear and logical
- Ready for significant feature expansion
```

### After Phase C (Production-Ready)
```
- Functions: all ≤60lines
- DRY violations: eliminated
- Documentation: comprehensive
- Developer tooling: complete
- Team-ready: onboarding docs and standards
```

---

## Document Index

- **Audit Findings**: `docs/audit_report_comprehensive.md`
- **Phase A (Stabilization)**: `docs/planning/plan_a_emergency_stabilization.md`
- **Phase B (Systematic)**: `docs/planning/plan_b_systematic_module_splitting.md`
- **Phase C (Polish)**: `docs/planning/plan_c_quality_enhancement.md`
- **FFmpeg-next Migration Plan**: `FFMPEG_MIGRATION_RECOMMENDATIONS.md`
- **This Overview**: `docs/planning/refactoring_roadmap_summary.md` 