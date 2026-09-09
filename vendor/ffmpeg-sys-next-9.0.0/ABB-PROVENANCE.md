# ABB FFmpeg sys build patch

Base: crates.io `ffmpeg-sys-next` 9.0.0 archive, SHA-256
`9b939bf79dd5949412a4b81cfe21a07f48ea21b47fcbb5f57816c8c2de5ae30b`.
The upstream manifest declares WTFPL.

ABB retains FFmpeg tag `n9.0`, verified at commit
`d32b387f2b0a484599d4587d651891f0c63c4238`, and the existing CoreAudio link
and portable-build support. Native build caches and the Linux environment
setup record the source revision plus patch hash before reusing libraries.

`patches/mov-chapter-start.patch` initializes a generated QuickTime chapter
track's first DTS as unknown, matching ordinary tracks. Otherwise the muxer
resets a nonzero first chapter start. The media regression prepends chapterless
audio to a chaptered M4B and verifies accepted chapter names and positions,
plus the QuickTime edit and sample timing tables.

This patch does not advance FFmpeg or change encoder selection or options.
Unbundled builds must use the same source and patch to satisfy the media proof.
