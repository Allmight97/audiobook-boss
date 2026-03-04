# Command Versioning Cleanup: Canonical Naming Plan

**Branch:** `feat/command-versioning-policy`
**Status:** Approved for execution
**Risk Level:** Low (internal naming only, single-user codebase)
**Estimated Files:** 10-12 files across Rust/TypeScript/docs

---

## Executive Summary

Eliminate legacy `v2` naming from the audio processing command (`process_audiobook_files_v2` → `process_audiobook_files`). This is not a breaking change—there is no v1 command in existence. The `v2` suffix is vestigial from an earlier architecture iteration and creates confusion with Tauri v2 framework references.

**What This Is:** Technical debt cleanup—renaming internal APIs to match current reality
**What This Is NOT:** API versioning strategy, deprecation policy, or breaking change management

---

## Motivation

1. **Confusion with Tauri v2:** Multiple "v2" concepts (framework version vs command version)
2. **No actual v1 exists:** The `v2` suffix implies a migration path that never existed
3. **Single-user codebase:** No external consumers to break; zero compatibility risk
4. **Canonical state:** Code and docs should describe current reality, not historical iterations

---

## Scope

### IN SCOPE (Remove v2 naming)
- `process_audiobook_files_v2` → `process_audiobook_files`
- `ProcessV2Payload` → `ProcessPayload`
- All related comments referencing "v2 command"
- Documentation updates to remove v2 references
- Test updates for new naming

### OUT OF SCOPE (Keep as-is)
- **Tauri v2 framework references** (legitimate framework version)
- **Encoder "v2" terminology** (describes enhanced encoder feature level, not versioning)
- Any functional changes to processing logic

---

## Files to Modify

### Rust Backend (4 files)
1. `src-tauri/src/commands/audio.rs` - Function rename
2. `src-tauri/src/commands/audio_types.rs` - Payload struct rename
3. `src-tauri/src/ipc_contract.rs` - Command registration
4. `src-tauri/src/commands/audio_processing.rs` - Import/reference updates

### TypeScript Frontend (4 files)
5. `src/types/audio.ts` - Type definition
6. `src/lib/tauri/client.ts` - Command invoker
7. `src/lib/tauri/normalizers.ts` - Normalization functions
8. `src/lib/generated/tauri.ts` - Auto-regenerated bindings

### Tests (2 files)
9. `src/lib/behavior-contract.test.ts` - Expected command names
10. `src/lib/tauri-client.test.ts` - Test assertions

### Documentation (3-4 files)
11. `docs/external-apis/tauri-commands.md`
12. `docs/specs/technical-reference.md`
13. `docs/decisions/004-ffmpeg-next-core-cli-escape-hatch-policy.md`
14. `src/lib/tauri/AGENTS.md` - Add "no versioning" policy

---

## Standards Check Loop

Four verification gates enforce quality:

```bash
# Gate 1: After code renaming
scripts/checks.sh standard

# Gate 2: After comment cleanup
scripts/checks.sh standard

# Gate 3: After documentation updates
cargo doc --no-deps

# Gate 4: After test updates
scripts/checks.sh standard

# Final verification
grep -r "process_audiobook_files_v2\|ProcessV2Payload" --include="*.rs" --include="*.ts" --include="*.md" src/ docs/ || echo "Clean"
bash scripts/check-generated-bindings.sh --mode local
```

---

## Success Criteria

- [ ] All `v2` references removed from command naming (except Tauri v2 framework, encoder v2 feature)
- [ ] `scripts/checks.sh standard` passes at all four gates
- [ ] Documentation reflects canonical naming
- [ ] AGENTS.md updated with "no versioning" policy
- [ ] Zero functional changes (pure rename)
- [ ] Single atomic commit with clear message

---

## Rollback

Not applicable. This is a single atomic commit on a feature branch. If issues arise, abandon branch and recreate.

---

## Context for Reviewer

This work eliminates a confusing naming artifact. The `v2` suffix on the processing command was added during a major refactor but:
- No v1 command ever existed to migrate from
- The suffix creates ambiguity with Tauri v2 framework references
- This is a single-user personal tool—breaking changes are acceptable and expected
- The encoder "v2" types are kept intact (they describe a feature level, not version migration)

**Key insight:** The user has zero tolerance for compatibility safety theater. This is greenfield cleanup, not production software maintenance.

---

## Related

- GitHub Issue: #248 (IPC/Connective Elements Improvements)
- Branch: `feat/command-versioning-policy`
- AGENTS.md: `src/lib/tauri/AGENTS.md` (to be updated with policy)
