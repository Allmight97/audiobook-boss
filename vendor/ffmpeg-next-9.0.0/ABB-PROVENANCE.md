# ABB wrapper compatibility patch

Base: the unmodified crates.io `ffmpeg-next` 9.0.0 archive, SHA-256
`6380599799e175191eb7ffe82c97f36a2a90a36cbc54c738a903e5287d7f516a`.
The original license and source remain included. Registry bookkeeping, upstream
CI configuration, and the crate-local lockfile are omitted.

This patch adds exact Rust variants and bidirectional mappings for the selected
FFmpeg source `705286a8a7a8f9118465b2bd83f99a6f066dcbbc`:

- `AV_SAMPLE_FMT_DSD`
- `AV_PIX_FMT_CUARRAY`
- `AV_FRAME_DATA_DOWNMIX_MATRIX`
- `AV_CODEC_ID_PCM_DVDA` and `AV_CODEC_ID_ITUT_T35`

No fallback mapping or unknown-enum panic is introduced. The vendor is tied to
ABB's pinned FFmpeg headers, including for unbundled developer builds. Retire
this patch when an upstream wrapper release represents this source ABI.
