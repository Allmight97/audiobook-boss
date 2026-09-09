# ABB FFmpeg sys build patch

Base: crates.io `ffmpeg-sys-next` 9.0.0 archive, SHA-256
`9b939bf79dd5949412a4b81cfe21a07f48ea21b47fcbb5f57816c8c2de5ae30b`. The upstream manifest declares WTFPL.

ABB builds FFmpeg from immutable commit
`705286a8a7a8f9118465b2bd83f99a6f066dcbbc`; source stamps include that
revision and the local patch blob hash, invalidating cached libraries when
either changes. The local sys changes also retain
the required CoreAudio framework link and distinguish portable distribution
builds from native development builds.

The existing header-only CUDA shim adds `CUarray` and
`CUDA_ARRAY3D_DESCRIPTOR` declarations needed to parse this FFmpeg revision.
Their layout was compared with FFmpeg/nv-codec-headers commit
`eddcea9e27f6b772057c9b3f87de2cc1737faffc`: on the compiling arm64 host the
descriptor is 40 bytes, alignment 8, with field offsets 0/8/16/24/28/32.
These declarations support binding generation; they add no CUDA runtime.

Source changes, wrapper enum coverage, and header/runtime versions must be
checked together when advancing FFmpeg. Real-media proof belongs to the Audio
owner; packaging qualification belongs to the release lane.

`patches/mov-chapter-start.patch` initializes a generated QuickTime chapter
track's first DTS as unknown, matching ordinary tracks. Otherwise the muxer
mistakes its first packet for a continuing fragment and resets a later first
chapter to zero. The media execution regression prepends chapterless audio to
a chaptered M4B and verifies the accepted names, starts and ends unchanged;
QuickTime edit/sample-table assertions also protect the player-visible track.
