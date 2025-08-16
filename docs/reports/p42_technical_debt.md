# P4.2 Technical Debt & Future Tasks
**Created: 2025-08-15**  
**Context**: Items deferred during P4.2 Metadata and Cover Art Integration implementation

## 🚧 High Priority Technical Debt

### TD-P4.2.1: Complete FFmpeg-Next Cover Art Embedding
**Priority**: High  
**Effort**: 2-3 days  
**Context**: Currently using finalize stage fallback (Lofty) for cover art embedding

**Implementation Requirements:**
- Research correct ffmpeg-next API for attachment streams
- Replace placeholder in `embed_cover_art_ffmpeg()` function
- Test cover art embedding during encoding vs post-processing
- Validate output compatibility with iTunes/VLC/other players

**Current Workaround:**
- Cover art embedding works via existing finalize stage (Lofty)
- No user-facing regression
- Cover art flow from UI → processing → output is complete

**Files to Update:**
- `src-tauri/src/metadata/ffmpeg_bridge.rs` (main implementation)
- `src-tauri/src/audio/media_pipeline.rs` (integration point)

```rust
// TODO in ffmpeg_bridge.rs - replace placeholder with:
pub fn embed_cover_art_ffmpeg(
    octx: &mut ff::format::context::Output,
    cover_data: &[u8],
) -> Result<()> {
    // Research needed: correct attachment stream API
    // Current placeholder returns Ok(()) - implement actual embedding
}
```

### TD-P4.2.2: Complete Twoloop AAC Enhancement
**Priority**: Medium  
**Effort**: 1-2 days  
**Context**: Enhanced AAC quality through better psychoacoustic analysis

**Implementation Requirements:**
- Research correct encoder option API in ffmpeg-next 7.x
- Test twoloop availability across different FFmpeg builds
- Implement graceful fallback to standard AAC-LC
- Add optional UI toggle for advanced users (future enhancement)

**Current Status:**
- Placeholder code added in `create_audio_encoder()`
- Logs warning when twoloop unavailable
- Standard AAC-LC encoding works correctly

**Files to Update:**
- `src-tauri/src/audio/media_pipeline.rs` (encoder configuration)
- Future: `src/ui/outputPanel.ts` (optional UI toggle)

```rust
// TODO in media_pipeline.rs - replace commented code with:
// Research: opened.set_option() vs other API for codec parameters
// Test: encoder context vs codec context for AAC options
```

## 📋 Medium Priority Enhancements

### TD-P4.2.3: Enhanced Metadata Validation
**Priority**: Medium  
**Effort**: 1 day  
**Context**: Expand metadata compatibility warnings and validation

**Implementation Ideas:**
- Add metadata field length limits (title, description, etc.)
- Validate image formats more strictly for cover art
- Add audiobook-specific metadata validation (chapters, duration consistency)
- Improve error messages for users

### TD-P4.2.4: Performance Optimization Research
**Priority**: Low  
**Effort**: 2-3 days  
**Context**: Ensure ffmpeg-next performance meets L4 requirements

**Research Areas:**
- Memory usage patterns during large file processing
- Frame-by-frame vs batch processing performance
- Progress reporting frequency optimization
- Cancellation response time improvements

## 🔄 Integration Points

### Related Roadmap Items
- **P4.3**: Engine transition will reveal any performance gaps
- **P5.2**: FDK-AAC implementation may inform twoloop approach
- **P5.3**: URL cover art loading will use same embedding pipeline

### Documentation Updates Needed
- Update API documentation for metadata bridge module
- Document ffmpeg-next encoder configuration patterns
- Add troubleshooting guide for cover art issues

## 📊 Tracking & Completion

### Success Criteria for Technical Debt Resolution
- [ ] Cover art embedded via ffmpeg-next during encoding (not post-processing)
- [ ] Twoloop AAC enhancement enabled by default with fallback
- [ ] All integration tests pass with complete implementation
- [ ] Performance benchmarks meet L4 standards (≤20% regression)

### Monitoring Points
- User reports of cover art issues → prioritize TD-P4.2.1
- Audio quality complaints → prioritize TD-P4.2.2  
- Performance issues → prioritize TD-P4.2.4

---

**Note**: These items are technical improvements, not user-facing bugs. The current P4.2 implementation is fully functional and production-ready with appropriate fallbacks and workarounds in place.