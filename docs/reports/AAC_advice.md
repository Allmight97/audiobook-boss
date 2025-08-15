# Encoder strategy and quality safeguards (pre-production)
- Default codec/path
  - Use ffmpeg-next with native AAC (LC), mono, 44.1/48 kHz, ABR/CBR at 80–96 kbps.
  - Default preset: 96 kbps (speech-safe). Offer alternatives: 80 kbps (“balanced”), 64 kbps (“smaller file”).
- Optional FDK support
  - Do not bundle FDK. At runtime, detect if libfdk_aac is available in the system FFmpeg build and expose it as “FDK-AAC (system)”.
  - Show an info tooltip: “Higher quality at low bitrates. Uses your system’s encoder; not bundled.”
- Runtime detection (ffmpeg-next)
  - Enumerate encoders via libavcodec; cache capabilities:
    - If encoder name contains “libfdk_aac” → mark FDK available.
    - Always include “aac” (native).
  - UI: Encoder dropdown with Auto = Native AAC; show “FDK detected” badge if present.
- Quality parity gate
  - Build a 10-sample corpus (8 speech, 2 stress music). Include sibilance, breaths, quiet passages, noisy room.
  - Encode matrix: Native AAC and FDK at 64/80/96 kbps mono.
  - Acceptance: For speech, native @ 80–96 kbps must be ABX-indistinguishable from FDK on majority of trials; if not, set default to 96 kbps.
  - Automate as a script + manual ABX checklist (human-in-the-loop).
- Tuning checks (native AAC)
  - Force LC profile, mono layout, desired bitrate.
  - Verify twoloop coder is active (default in modern FFmpeg). Lock sample rate to 44.1 or 48 kHz.
  - Consistent downmix to mono before encode.

# Roadmap integration (add to P1/P2)
- P1.x: Encoder selection seam
  - Create Encoder trait and two implementations: NativeAacEncoder, FdkAacEncoder.
  - Add runtime encoder registry + selection logic (env override for tests).
  - Unit test: registry detects available encoders; falls back to native.
- P2.3: Parity tests
  - Implement corpus-driven encode harness and comparison:
    - Objective checks: duration match, RMS level drift < 0.5 dB, no channel/layout drift.
    - Store encodes; provide ABX instructions output (links to files).
  - CI job runs objective checks; ABX is documented/manual sign-off before engine flip.
- P2.1: Remove ShellFFmpeg
  - Delete shell path; ensure ffmpeg-next handles both encoders behind feature/registry.
  - Packaging: confirm no external FFmpeg binary bundled on platforms where policy forbids it.

# Legal/packaging guardrails
- Do not link or ship libfdk_aac.
- Add docs page: “Why FDK isn’t bundled; how to enable via system FFmpeg.”
- On first toggle of FDK in UI, show informational notice: “Uses system-provided encoder. Licensing is your responsibility.”

# User-facing presets
- Presets:
  - Max quality (default): 96 kbps, LC, mono.
  - Balanced: 80 kbps, LC, mono.
  - Smaller file: 64 kbps, LC, mono.
- Advanced option (when detected): Encoder = Native AAC (default) or FDK-AAC (system).

# Minimal code tasks (bite-sized)
- Add encoder registry (Rust): enumerate encoders; expose is_fdk_available().
- Implement NativeAacEncoder using ffmpeg-next; lock LC/mono/bitrate.
- Implement FdkAacEncoder guarded by runtime availability.
- UI: Add Encoder dropdown + Presets.
- Add encode harness script for corpus.
- Write docs: encoder selection, licensing note, how to install FDK-enabled FFmpeg.

# Exit criteria to ship
- Native AAC passes ABX gate for speech at 96 kbps on corpus; defaults set accordingly.
- Shell path removed; ffmpeg-next path stable.
- Runtime FDK detection works; UI toggle appears only when available.
- Docs updated with legal note and user guidance.

If you want, I can draft:
- The Encoder trait + registry skeleton (Rust).
- The corpus harness script and the ABX checklist template.