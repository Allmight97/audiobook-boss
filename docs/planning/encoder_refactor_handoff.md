# Next Session: Split encoder.rs + Test Remaining Encoders

## Handoff Summary (This Session)

### What We Did

Fixed **Issue #49: FDK HE-AAC VBR ignored** - VBR level changes were being ignored, output was always ~64 kbps.

### Root Cause

The previous code used `av_opt_set_int()` to set encoder options (vbr, profile, afterburner), then called `open_as(codec)`. This failed because **encoder-private options must be passed through `avcodec_open2()` via a Dictionary**, not set directly on the context beforehand.

### Solution

Switched to ffmpeg-next's `open_as_with(codec, options)` API which passes the Dictionary to `avcodec_open2()` - exactly how FFmpeg CLI handles `-vbr 3 -profile:a aac_he -afterburner 1`.

### Changes Made

- **Removed**: `configure_fdk_encoder()`, `configure_aac_at_encoder()`, `configure_native_aac_encoder()` functions that used `av_opt_set_int()`
- **Added**: `build_fdk_options()`, `build_apple_options()`, `build_native_options()` that return `Dictionary`
- **Changed**: `create_audio_encoder()` now calls `open_as_with(codec, opts)` instead of `open_as(codec)`
- **Removed**: Unused `try_enable_twoloop_aac()` function
- **Updated**: Test to verify FDK Dictionary contains correct options

### Verification

- FDK VBR tested: levels 1, 3, 5 now produce different output bitrates
- `./scripts/quick-checks.sh` passes
- `cargo test fdk_options` passes

### Impact

- FDK HE-AAC VBR now works correctly (closes #49)
- Architecture aligns with how FFmpeg CLI handles encoder options
- Apple and Native AAC paths updated but **not yet tested**

---

## Next Session Tasks

### 1. Test Apple AAC Encoder

Verify `aac_at` CVBR mode works correctly with the new Dictionary approach.

### 2. Test Native AAC Encoder

Verify native FFmpeg AAC with `aac_coder=twoloop` via Dictionary works correctly.

### 3. Split encoder.rs (849 LOC → <400 LOC each)

Current file exceeds AGENTS.md complexity limit (400 LOC max).

**Proposed module structure**:

```
src-tauri/src/audio/processor/encoder/
├── mod.rs          (~100 LOC) - Public API, re-exports
├── context.rs      (~120 LOC) - create_audio_encoder, setup_encoder
├── options/
│   ├── mod.rs      (~20 LOC)  - Re-exports
│   ├── fdk.rs      (~50 LOC)  - build_fdk_options + docs
│   ├── apple.rs    (~40 LOC)  - build_apple_options + docs
│   └── native.rs   (~50 LOC)  - build_native_options + docs
├── common.rs       (~100 LOC) - resolve_target_audio_params, probe_first_input, configure_threads
├── write.rs        (~120 LOC) - encode_and_write_frame, finalize_encoding
└── tests.rs        (~150 LOC) - All tests (or keep inline)
```

**Key files to touch**:

- [`src-tauri/src/audio/processor/encoder.rs`](src-tauri/src/audio/processor/encoder.rs) - Split into module
- [`src-tauri/src/audio/processor/mod.rs`](src-tauri/src/audio/processor/mod.rs) - Update module declaration

**Approach**:

1. Create `encoder/` directory
2. Move functions to appropriate submodules
3. Update visibility and imports
4. Verify all tests pass
5. Run quick-checks
