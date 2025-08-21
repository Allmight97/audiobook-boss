### Afterburner Performance Impact
The `-afterburner` flag in FFmpeg's native AAC encoder enables post-processing to refine the encoded audio, which improves quality (e.g., better handling of transients and reduced artifacts) but comes at a minor cost to encoding speed. Based on official docs and user reports:[1][2]

- **Speed decrease**: It typically increases encoding time by 5-15% compared to disabling it (`-afterburner 0`), depending on hardware, audio complexity, and bitrate. For example, in tests with speech content, enabling afterburner added about 10% to CPU time on average systems, but this is often negligible for short files—think seconds rather than minutes for a typical audiobook chapter. It's not a huge hit because it's a lightweight refinement step, not a full re-encode.[3]
- **Why the trade-off?** Docs note it enhances objective metrics like PSNR without massive overhead, and users confirm it's worth it for quality unless you're optimizing for ultra-fast batch processing. If your audiobooks are long (e.g., hours), disabling it could shave off meaningful time cumulatively, but for most users, the quality boost justifies the small slowdown.[4][1]

If performance is a big concern, test with and without it on your Mac (since you're on macOS)—use tools like `time` in terminal to measure real-world differences on sample files.

### Quality Loss with Performance Settings and Bitrate Compensation
Yes, realistically, you'll likely still notice some quality loss when using the fast coder (0) over twoloop (1), even if you bump the bitrate to 80k and optimize for performance. However, for audiobook speech (which is less complex than music), the difference is often subtle and may not be bothersome unless you're doing critical listening or A/B tests. Here's why, based on reports:[5][6][7]

- **Compensation limits**: Increasing bitrate helps mitigate artifacts (e.g., from 64k to 80k can add clarity and reduce muddiness), but the fast coder's simpler quantization doesn't adapt as well to audio nuances like sibilance or pauses in narration. User tests at low-to-mid bitrates (e.g., 80-128k) show twoloop retaining better detail and "naturalness," while fast can introduce slight distortion or flatness, even at higher rates. For stereo-to-mono downmixes from 92-128k sources, you'd lose more from fast coder than the bitrate bump gains back—think a 10-20% subjective quality drop in blind tests for speech.[7][8][9][10][5]
- **Your goal alignment**: At 80k mono HE-AAC, you'll achieve good space efficiency (e.g., halving file size from stereo originals) while keeping encode times reasonable. Quality retention is high with twoloop + afterburner, but if you go full performance (fast + no afterburner), expect minor but audible degradation like reduced crispness in voices. It's not "terrible"—many users find it acceptable for casual listening—but not as transparent as your original 92-128k files.[4][5]

To minimize this without slow encodes, prioritize twoloop for quality variants and only use fast for speed-critical ones. Benchmark on your setup: encode a 5-minute sample and compare file sizes, times, and audio quality.

### Recommendations for Your App and Profiles
Given your app's distribution (public, cross-platform via ffmpeg-next Rust crate), avoiding FDK is smart—its licensing restricts commercial or broad distribution due to patent clauses that could require fees, and it's considered non-free by projects like Debian and Fedora. You can't bundle it without legal risks, but native AAC is fully open and distributable.[11][4]

For macOS users (like your MacBook), leverage the `aac_at` encoder (Apple's AudioToolbox)—it's native to macOS, offers better quality than FFmpeg's built-in AAC (closer to FDK levels, with less distortion at low bitrates), and can be hardware-accelerated for faster encodes. On Linux/Windows, fall back to native `aac`. This platform-specific approach fits your Rust backend: use conditional compilation in ffmpeg-next to select encoders based on OS, and lofty should handle metadata passthrough seamlessly as long as you use `-map_metadata 0`.[12][13][5][4]

Updated profiles below incorporate this. They target your goals: mono HE-AAC at 64-80k for space savings (e.g., ~30-50% smaller than 92-128k stereo), passthrough sample rate/metadata/chapters/art, and m4b output. I prioritized twoloop for quality where possible, with afterburner enabled by default for balance. Adjust based on tests.

```bash
# Variant 1: Max quality (uses twoloop + afterburner; aac_at on macOS for superior results)
# macOS: ffmpeg -i input.ext -map 0 -c:v copy -c:a aac_at -b:a 64k -ac 1 -profile:a aac_he -map_metadata 0 -f ipod output_64k_variant1.m4b
# Linux/Windows: ffmpeg -i input.ext -map 0 -c:v copy -c:a aac -b:a 64k -ac 1 -profile:a aac_he -aac_coder 1 -afterburner 1 -map_metadata 0 -f ipod output_64k_variant1.m4b
# (Repeat for 72k and 80k by changing -b:a)

# Variant 2: Max performance (uses fast coder, no afterburner; aac_at on macOS for speed boost)
# macOS: ffmpeg -i input.ext -map 0 -c:v copy -c:a aac_at -b:a 64k -ac 1 -profile:a aac_he -map_metadata 0 -f ipod output_64k_variant2.m4b  # aac_at is often faster via hardware
# Linux/Windows: ffmpeg -i input.ext -map 0 -c:v copy -c:a aac -b:a 64k -ac 1 -profile:a aac_he -aac_coder 0 -afterburner 0 -map_metadata 0 -f ipod output_64k_variant2.m4b
# (Repeat for 72k and 80k)

# Variant 3: Balance (twoloop + afterburner on macOS/native; fast on others if needed)
# macOS: ffmpeg -i input.ext -map 0 -c:v copy -c:a aac_at -b:a 64k -ac 1 -profile:a aac_he -map_metadata 0 -f ipod output_64k_variant3.m4b  # Balances via aac_at efficiency
# Linux/Windows: ffmpeg -i input.ext -map 0 -c:v copy -c:a aac -b:a 64k -ac 1 -profile:a aac_he -aac_coder 1 -afterburner 1 -map_metadata 0 -f ipod output_64k_variant3.m4b
# (Repeat for 72k and 80k)
```

These should encode quickly (e.g., real-time or faster on modern hardware) while hitting your quality/space targets. If macOS performance isn't enough, experiment with disabling afterburner in native fallback, but aac_at often outperforms native for both speed and quality. Let me know if you need Rust code snippets for integration![5][12]

[1] https://ffmpeg.org/ffmpeg-codecs.html
[2] https://www.mankier.com/1/ffmpeg-codecs
[3] https://github.com/blakeblackshear/frigate/discussions/11498
[4] https://trac.ffmpeg.org/wiki/Encode/AAC
[5] https://www.reddit.com/r/ffmpeg/comments/1lc41k6/why_native_aac_is_considered_worse_than_aac_at/
[6] https://www.reddit.com/r/ffmpeg/comments/oa9f0x/ffmpeg_audio_aac_vs_opus/
[7] https://www.voukoder.org/forum/thread/574-premiere-pro-voukoder-aac-sound-quality-issue/
[8] https://forum.videohelp.com/threads/374701-FFmpeg-s-AAC-encoder-received-major-improvements
[9] http://archimago.blogspot.com/2023/08/part-ii-comparison-of-bluetooth.html
[10] https://news.ycombinator.com/item?id=29680105
[11] https://wiki.hydrogenaudio.org/index.php?title=Fraunhofer_FDK_AAC
[12] https://stackoverflow.com/questions/63460919/how-to-improve-the-output-video-quality-with-ffmpeg-and-h264-videotoolbox-flag
[13] https://www.audiobookshelf.org/guides/ffprobe/
[14] https://trac.ffmpeg.org/ticket/2686?cnum_hist=369&cversion=1
[15] https://linuxiac.com/ffmpeg-7-1-promises-major-improvements-in-video-processing/
[16] https://www.audiophile-heaven.com/2023/03/bluetooth-sound-quality-guide-what-are-the-codecs-and-how-do-they-work.html
[17] https://mike.gold/notes/x-bookmarks/open-source/fraunhofer-aac-vs-ffmpeg-aac