# Issue #71: Batch Processing Bug Fixes

**Status**: Ready for implementation
**PR Strategy**: 2 PRs (Phase 1 correctness, Phase 2 UX default change)
**Author**: Claude Code + jstar
**Date**: 2024-12-20 (amended 2024-12-21)

---

## Context

- **Sole dev/user/PM**: jstar - no external users, no backward compatibility concerns
- **Approach**: Direct breaking changes allowed, aggressive cleanup encouraged
- **Branch**: `feat/issue71_fix_multi_jobs`

## Product Decisions (explicit)

- **Merge mode uses current form metadata** (assumes all inputs are chapters of one book).
- **Batch mode uses per-file metadata** (each file retains its own metadata, editable per file).

UX impact: merge stays simple and predictable for a single title; batch becomes correct and transparent, with each file's metadata preserved and editable.  
DX impact: a single explicit metadata map keyed by file path removes ambiguity and makes debugging deterministic.

## Problem Summary

| Bug | Symptom | Root Cause |
|-----|---------|------------|
| Metadata bleeding | All batch files get same metadata | `commands/audio.rs:229` clones single metadata for all jobs |
| Wrong progress display | Shows "Input X of Y" | `frame_pipeline.rs:135-139` emits counter, not filename |

## Tri-Order Impact Analysis

### First Order (Direct)
- Fix metadata per-file in batch mode
- Fix progress display to show actual filename + index

### Second Order (Adjacent)
- Frontend needs per-file metadata state management
- Command contract changes (TS ↔ Rust boundary)

### Third Order (Systemic)
- Establishes pattern for per-file state management
- Prepares codebase for Phase 2 UX redesign

---

## Phase 1: Bug Fixes (PR1)

### Step 1: Backend - Replace Metadata Parameter (breaking change)

**File:** `src-tauri/src/commands/audio.rs`

**Change signature directly** (no optional fallback - breaking change is fine):

```rust
// FROM (line 128):
metadata: Option<crate::metadata::AudiobookMetadata>,

// TO:
metadata: Option<std::collections::HashMap<String, crate::metadata::AudiobookMetadata>>,
```

**Update merge mode** (lines 180-202, deterministic lookup):
```rust
// For merge, use the first input file path as the key
let merge_key = payload.input_files.first().map(|s| s.as_str());
let merge_metadata = merge_key
    .and_then(|key| metadata.as_ref().and_then(|map| map.get(key).cloned()));
// Pass merge_metadata to run_processing_job
```

**Update batch loop** (line 229):
```rust
// FROM:
let md_cloned = metadata.clone();

// TO:
let md_cloned = metadata.as_ref()
    .and_then(|map| map.get(input).cloned());
```

**Contract note**: `metadata` keys **must** match `payload.input_files` exactly.

### Step 2: Backend - Fix Progress Filename

**File:** `src-tauri/src/audio/processor/frame_pipeline.rs`

Add field to `FramePipelineCtx` (after line 115):
```rust
pub(crate) current_file_name: String,
```

Update `emit_progress_update` (lines 135-139):
```rust
Some(format!(
    "{} ({}/{})",
    ctx.current_file_name,
    ctx.current_file_index + 1,
    ctx.total_files
)),
```

**File:** `src-tauri/src/audio/processor/engine.rs`

Set filename before processing each file in the loop:
```rust
ctx.current_file_name = in_path.file_name()
    .and_then(|n| n.to_str())
    .unwrap_or("unknown")
    .to_string();
```

### Step 3: Frontend - Per-File Metadata State

**New File:** `src/ui/metadataState.ts`

```typescript
import type { AudiobookMetadata } from '../types/metadata';

const store = new Map<string, Partial<AudiobookMetadata>>();

export const metadataState = {
  set(filePath: string, metadata: Partial<AudiobookMetadata>): void {
    store.set(filePath, metadata);
  },

  get(filePath: string): Partial<AudiobookMetadata> | undefined {
    return store.get(filePath);
  },

  getAll(): Record<string, Partial<AudiobookMetadata>> {
    return Object.fromEntries(store);
  },

  clear(): void {
    store.clear();
  },

  has(filePath: string): boolean {
    return store.has(filePath);
  }
};
```

### Step 4: Frontend - Update Command Invocation

**File:** `src/ui/statusPanel/logic.ts` (lines 284-293)

```typescript
import { metadataState } from '../metadataState';

// Build per-file metadata map
const metadataMap = metadataState.getAll();

// Merge mode: use current form metadata for the first input file key
if (v2Payload.jobType === "merge" && v2Payload.inputFiles.length > 0) {
  metadataMap[v2Payload.inputFiles[0]] = getCurrentMetadataFromForm();
}

// Batch mode: prefill missing metadata by reading from disk before invoking
// (use read_audio_metadata for any input file lacking state)

const result = await bridge.invoke("process_audiobook_files_v2", {
    payload: v2Payload,
    metadata: Object.keys(metadataMap).length > 0 ? metadataMap : null,
    previewSeconds: options?.previewSeconds,
});
```

### Step 5: Frontend - Wire Metadata State to File Selection

**File:** `src/ui/fileList/` (or wherever file selection is handled)

On file selection change:
```typescript
// Save current form to state for previous file
if (previousFilePath) {
  metadataState.set(previousFilePath, getCurrentMetadataFromForm());
}

// Load from state for new file (or auto-extract if not present)
const stored = metadataState.get(newFilePath);
if (stored) {
  populateFormFromMetadata(stored);
} else {
  // Auto-extract and store
  const extracted = await bridge.invoke('read_audio_metadata', { filePath: newFilePath });
  metadataState.set(newFilePath, extracted);
  populateFormFromMetadata(extracted);
}
```

On files cleared:
```typescript
metadataState.clear();
```

---

## Phase 1.5: Documentation Sync (post-PR1, pre-PR2)

**Goal**: Update API documentation to reflect the breaking command contract change.

**Required Docs (start here):**
- `docs/external-apis/README.md`
- `docs/external-apis/tauri-commands.md`
- `docs/external-apis/tauri-ts-boundaries.md`
- `docs/external-apis/tauri-patterns.md` (if progress payload shape changes)

**Changes to capture:**
- `process_audiobook_files_v2` now accepts a per-file metadata map keyed by input path.
- Merge mode uses current form metadata for the first input file key.
- Progress messages now include filename + index.

---

## Critical Files

| File | Change Type | Lines |
|------|-------------|-------|
| `src-tauri/src/commands/audio.rs` | Modify | 128, 180-202, 229 |
| `src-tauri/src/audio/processor/frame_pipeline.rs` | Modify | 105-122, 135-139 |
| `src-tauri/src/audio/processor/engine.rs` | Modify | ~172-210 |
| `src/ui/metadataState.ts` | **NEW** | - |
| `src/ui/statusPanel/logic.ts` | Modify | 284-293 |
| `src/ui/fileList/` | Modify | file selection handler |
| `docs/external-apis/*` | Modify | API contract updates |

---

## Testing Checklist (owner-required)

**Manual testing (jstar) — REQUIRED before merge:**
- [ ] Load 3 files with different existing metadata
- [ ] Switch between files, verify metadata persists per-file
- [ ] Process in batch mode (convert each separately)
- [ ] Verify each output file has correct unique metadata
- [ ] Verify progress shows "filename.m4b (1/3)" format
- [ ] Verify merge mode still works (uses first file's metadata)

Automated:
- [ ] `scripts/quick-checks.sh` passes
- [ ] `scripts/ensure-contract.sh` passes (update TS types for new signature)
- [ ] `cargo test` passes
- [ ] External API docs updated (see Phase 1.5)

---

## Phase 2: UX Redesign (PR2)

After Phase 1 is stable:
- Change default job type from "merge" to "batch"
- Make merge an opt-in toggle ("Merge chapters into single book")
- Consider metadata manager UI improvements for multi-file workflow
- Remove legacy `is_processing` mutex (use JobRegistry only)

---

## Design Decisions

### Why HashMap<String, Metadata> instead of Vec?
- O(1) lookup by file path in batch loop
- Natural key-value semantics for "this file → this metadata"
- Avoids index synchronization issues between file list and metadata list

### Why break the API instead of adding optional param?
- Sole dev/user - no external consumers
- Simpler implementation - one code path, not two
- Cleaner contract - no ambiguity about which param takes precedence

### Why not auto-extract all metadata on import?
- Could be slow for large file lists
- Lazy extraction on file selection is more responsive
- User can still bulk-edit common fields (author, series) across files

### PR sequencing (recommended)
- PR1 from current branch → review → merge → quick smoke test on `main`
- PR2 from updated `main` → review → merge → smoke test

---

## Rollback Plan

If Phase 1 introduces regressions:
1. Revert PR
2. Debug on branch
3. Re-submit

No user impact concerns - jstar is sole user.
