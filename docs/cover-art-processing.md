# Cover Art Processing

Read this when changing how covers are loaded, passed through, converted, or
written onto an output M4B. Ordinary tag work stays in
`src-tauri/src/metadata/AGENTS.md`.

## Write contract

Passthrough means keep a cover that already meets the write target. The target
is JPEG at or under 800px. PNG, oversized JPEG, and other decodeable images are
converted once by `optimize_cover_art` after merge. Both encoder paths then mux
and write the same JPEG.

User-picked file and URL covers are already converted at command ingress
(`load_cover_art_file`, `load_cover_art_from_url`). Source-embedded covers are
not converted at import. Conversion happens at write-prep so display reads stay
raw source truth.

## Flow

```mermaid
flowchart TD
  drop[Drop or pick audio] --> inspect[Audio inspect: duration, tags, format]
  drop --> listThumb[File-list thumbnail: bounded 64px JPEG]
  drop --> hydrate[Inspector: raw source cover for display]
  hydrate --> intent{User replaced cover?}
  intent -->|File or URL| loadOpt[optimize_cover_art at command ingress]
  intent -->|No| plan[plan_metadata_outcome]
  loadOpt --> plan
  inspect --> plan
  plan --> engine[Audio engine: native ffmpeg-next or external FDK]
  engine --> merge[merge_passthrough_cover_art]
  merge --> prep[prepare_output_cover_art once]
  prep -->|JPEG already at or under 800px| keep[Keep bytes]
  prep -->|PNG or oversized JPEG| jpeg[optimize_cover_art]
  keep --> sinks{Encoder path}
  jpeg --> sinks
  sinks -->|Native| mux[Mux JPEG attached_pic during encode]
  sinks -->|FDK| encode[Encode audio only, no cover]
  encode --> remux[finalize_artifact_metadata remux JPEG]
  mux --> mp4[mp4ameta covr and tags]
  remux --> mp4
  mp4 --> commit[output_artifact commit]
```

## Owners

| Step | Owner |
| --- | --- |
| File-list thumbnail | Metadata thumbnail / `mp4_covr` |
| Inspector display | Metadata read; frontend cache |
| User-picked cover ingest | Commands call `optimize_cover_art` |
| Include vs suppress after clear | `CoverArtPassthroughPolicy` |
| Write-ready bytes | `prepare_output_cover_art` after merge |
| FFmpeg attached_pic mux | Metadata `embedding.rs` |
| MP4 `covr` atom | mp4ameta |
| Final file move | `output_artifact` |

Do not convert in the FFmpeg encoder-open helper, the file-list loader, or
`CoverArtPassthroughPolicy`. That policy only answers include vs suppress.
