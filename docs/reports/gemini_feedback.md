# Audit Report: New UI Plan

## Overall Assessment
The plan `@docs/planning/newUI_plan.md` is **solid, well-researched, and accurate**. It correctly identifies the critical gaps between the current codebase and the desired state (mock). The proposed 2-PR strategy (Backend Foundation -> UI Replacement) is excellent for risk mitigation.

## Gap Verification

| Gap | Status | Findings |
| :--- | :--- | :--- |
| **Cover Art Optimization** | ✅ **Confirmed** | `src-tauri/src/commands/metadata.rs` validates extensions/headers but has **zero** resizing/conversion logic. The `image` crate addition is necessary. |
| **Bitrate Options** | ✅ **Confirmed** | `src-tauri/src/audio/settings_encoder.rs` and `src/types/audio.ts` both lack `104` and `120` kbps options. |
| **Preview Duration** | ✅ **Confirmed** | Backend command `process_audiobook_files_v2` **already accepts** `preview_seconds: Option<f64>`. The gap is purely frontend wiring (split button UI). |

## Technical Validation

### Tailwind CSS Strategy
The decision to use **npm integration** over CDN is **strongly endorsed**.
- **Offline Support**: Essential for a desktop app.
- **Performance**: Tree-shaking will significantly reduce the CSS bundle size compared to the full CDN build.
- **Maintainability**: `tailwind.config.js` allows for a centralized theme definition that matches the mock's variables.

### Backend Changes
- **`optimize_cover_art`**: The plan to implement this in `metadata.rs` is correct. Ensure it handles transparency in PNGs correctly when converting to JPEG (usually involves a white or black background fill, or just keeping it as PNG if transparency is detected, though the requirement says "Convert to JPG"). *Suggestion: If transparency is critical, consider keeping PNG for those cases, or explicitly flatten to white background.*
- **Bitrate Whitelist**: Trivial update, low risk.

### Frontend Changes
- **`tagPreview.ts`**: The suggestion to potentially create a new file for this logic is good. `outputPanel.ts` is already ~360 LOC and handling the grid logic there would clutter it.
- **Element IDs**: The plan explicitly mentions preserving element IDs. This is **critical** as `src/ui/*.ts` relies heavily on `getElementById`.

## Suggestions & Refinements

1.  **Sample Rate "Auto" Logic**:
    - Ensure that selecting "Auto" in the UI correctly passes `null` or `undefined` (or the specific enum variant) to the backend. The current `AudioSettings` interface uses `SampleRateConfig = 'auto' | { explicit: number }`. Verify the backend `ProcessV2Payload` deserialization handles this correctly (it maps to `Option<audio::SampleRateConfig>`).

2.  **Cover Art Optimization Edge Case**:
    - **Transparency**: Converting PNG to JPG will lose transparency. If the user drags in a transparent PNG, the background will become black (default for many converters) or white.
    - **Recommendation**: Explicitly decide on a background color (e.g., white) when flattening transparency, OR allow PNGs to remain PNGs if they are small enough, only converting if they exceed the size threshold. The current requirement says "If cover > 500KB OR format == PNG → Convert to JPG". This implies *all* PNGs get converted. Just be aware of the transparency loss.

3.  **Testing**:
    - Add a specific check for **HE-AAC v2 Stereo Coercion**. The backend `settings_encoder.rs` enforces stereo for HE-AAC v2. The UI should reflect this (disable Mono option when HE-AAC v2 is selected). The mock shows this logic in the "Advanced settings" section, but ensure the TypeScript logic enforces it too.

## Conclusion
The plan is ready for execution. No major blockers or architectural flaws found.
