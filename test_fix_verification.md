# M4B Cover Art Stream Fix - Verification Plan

## Issue Summary
- **Problem**: M4B containers reject MJPEG/PNG video streams for cover art
- **Error**: `[ipod @ 0x...] Could not find tag for codec none in stream #0, codec not currently supported in container`
- **Root Cause**: FFmpeg's "ipod" muxer doesn't support video streams for cover art in M4B containers

## Fix Implementation
1. **Detection**: Added M4B/MP4 container format detection
2. **Graceful Fallback**: Skip native cover art embedding for M4B containers
3. **Lofty Integration**: Use existing Lofty-based embedding in finalize stage
4. **Logging**: Clear messages when M4B fallback is triggered

## Verification Steps

### Test 1: M4B Processing (Primary Fix)
**Expected Behavior:**
- Cover art stream embedding is skipped
- Audio stream becomes stream #0 (no conflicts)
- Processing completes successfully
- Cover art is embedded via Lofty in finalize stage

**Log Messages to Look For:**
```
[INFO] M4B/MP4 container detected - deferring cover art embedding to finalize stage for better compatibility
[INFO] Audio processing completed with metadata integration
```

**Log Messages That Should NOT Appear:**
```
[ERROR] Could not find tag for codec none in stream #0
[INFO] Added cover art stream pre-header (index=0, format=Jpeg, bytes=...)
```

### Test 2: Other Container Formats
**Expected Behavior:**
- Cover art embedding via video streams should still work for non-M4B formats
- No regressions in existing functionality

### Test 3: Stream Indexing Verification
**Expected Result:**
- Audio stream should be at index 0 when no cover art stream is added
- No hardcoded stream index conflicts

## Success Criteria
- [ ] M4B processing completes without codec errors
- [ ] Audio quality is preserved
- [ ] Cover art is successfully embedded (via Lofty)
- [ ] No regression for other container formats
- [ ] Clear logging indicates which embedding method is used

## Files Modified
- `src-tauri/src/metadata/ffmpeg_bridge.rs`
  - `add_cover_art_stream_pre_header()`: Added M4B detection and fallback logic

## Next Steps After Verification
1. If successful: Document the approach and update user guidance
2. If issues remain: Consider alternative approaches (different codec IDs, metadata-only streams)
3. Performance testing with various M4B files and cover art sizes