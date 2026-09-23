# FAAC in ABB

Use this route for FAAC API decisions, source updates, patch retirement, and
encoding quality/performance comparisons. Ordinary encoder UI work follows
its local owner until an upstream behavior question needs evidence.

## Establish the selected behavior

Read `vendor/faac-sys/ABB-PROVENANCE.md` for the selected revision, local
changes, build configuration, and library boundary. Inspect `build.rs`, the
compiled public header, and `src-tauri/src/audio/processor/encoder/faac.rs`
for the actual ABI and adapter. Settings and capabilities own ABB's supported
choices; the library owns profile Auto resolution. Keep revision-specific
thresholds and current patch facts out of this skill.

Distinguish encoder Auto, FAAC profile Auto, and FAAC rate-control Auto. Trace
requested output rate, resolved channels, per-channel bitrate or VBR quality,
and opened encoder information. A source bitrate or a frontend label cannot
prove the chosen profile, bitrate mode, or playable interval.

## Evaluate an upstream update

Compare the selected commit with current upstream HEAD and inspect relevant
commits and issues. Record the exact candidate SHA. Separate codec changes,
ABI/build changes, concurrency repair, and platform-specific optimization;
only adopt changes relevant to the authorized ABB slice.

Retrieve upstream research into OS temp. When updating the owned vendor
slice, reconcile the upstream source list and headers with ABB's build, update
the build version and provenance together, and identify every retained local
change. Retire a patch when the candidate supplies its obligation and the
owning regression proof passes. A fix to library flushing or priming does not
by itself replace ABB's MP4/decoder handoff.

Use the existing sys and real-media lanes described in `scripts/AGENTS.md`.
Select proof for the changed risk: concurrent independent handles; both Auto
outcomes; mono/stereo signaling; short and partial final frames; Apple playback;
and ABB re-import sample count, alignment, and tail. Compare upstream source
files against the candidate archive so provenance distinguishes local changes
from a source selection. Keep release/license work with the release skill.

## Evaluate quality and performance

Compare narration and music/effects-rich samples at matched final size when
judging ABR against VBR or LC against HE. Record source rate/channels, requested
and resolved settings, encoded bytes, elapsed time, and decoder used. Reuse
existing diagnostic records; add a benchmark helper only if recurring work
justifies maintaining it.

Listening evidence, decoded timing/integrity, and throughput answer different
questions. A higher quality number is not a bitrate promise; a smaller file or
passing waveform check does not establish better sound. State which outcome
was measured. Keep full-quality defaults unless evidence supports an approved
tradeoff, and report unmeasured performance or listening quality explicitly.

Finish with the candidate/selected SHA, adopted changes, retired or retained
patches and why, ABB owner changes, proof performed, and remaining uncertainty.
