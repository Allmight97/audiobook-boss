
> audiobook-boss@0.1.0 tauri
> tauri dev

     Running BeforeDevCommand (`npm run dev`)

> audiobook-boss@0.1.0 dev
> vite


  VITE v6.3.5  ready in 180 ms

  ➜  Local:   http://localhost:1420/
     Running DevCommand (`cargo  run --no-default-features --color always --`)
        Info Watching /Users/jstar/Projects/audiobook-boss/src-tauri for changes...
   Compiling objc2-exception-helper v0.1.1
   Compiling ffmpeg-sys-next v7.1.3
   Compiling objc2 v0.6.1
   Compiling block2 v0.6.1
   Compiling objc2-core-foundation v0.3.1
   Compiling dispatch2 v0.3.0
   Compiling objc2-foundation v0.3.1
   Compiling objc2-core-graphics v0.3.1
   Compiling ffmpeg-next v7.1.0
   Compiling objc2-cloud-kit v0.3.1
   Compiling objc2-quartz-core v0.3.1
   Compiling objc2-core-image v0.3.1
   Compiling objc2-core-data v0.3.1
   Compiling objc2-app-kit v0.3.1
   Compiling objc2-web-kit v0.3.1
   Compiling tao v0.34.0
   Compiling muda v0.17.0
   Compiling window-vibrancy v0.6.0
   Compiling rfd v0.15.4
   Compiling wry v0.52.1
   Compiling tauri-runtime-wry v2.7.1
   Compiling tauri v2.6.2
   Compiling tauri-plugin-fs v2.4.1
   Compiling tauri-plugin-opener v2.4.0
   Compiling tauri-plugin-dialog v2.3.1
   Compiling audiobook-boss v0.1.0 (/Users/jstar/Projects/audiobook-boss/src-tauri)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 31.01s
     Running `target/debug/audiobook-boss`
[2025-08-18T15:52:46Z INFO  audiobook_boss_lib] Starting Audiobook Boss application
[2025-08-18T15:52:56Z DEBUG lofty::probe] Probe: Opening `/Users/jstar/Downloads/ABB Tests/01 - Introduction/01 - Introduction.mp3` for reading
[2025-08-18T15:52:56Z DEBUG lofty::probe] Probe: Guessed file type `Some(Mpeg)` from extension
[2025-08-18T15:52:56Z DEBUG lofty::id3::v2::header] Parsing ID3v2 header
[2025-08-18T15:52:56Z DEBUG lofty::id3::v2::read] Parsing ID3v2 tag, size: 75021, version: V3
[2025-08-18T15:52:56Z DEBUG lofty::id3] Searching for an ID3v1 tag
[2025-08-18T15:52:56Z DEBUG lofty::id3] Found an ID3v1 tag, parsing
[2025-08-18T15:52:56Z DEBUG lofty::id3] Searching for a Lyrics3v2 tag
[2025-08-18T15:52:56Z WARN  lofty::mpeg::properties] MPEG: Using bitrate to estimate duration
[2025-08-18T15:52:56Z DEBUG lofty::mpeg::properties] MPEG: VBR detected
[2025-08-18T15:52:56Z DEBUG lofty::probe] Probe: Opening `/Users/jstar/Downloads/ABB Tests/01 - Introduction/04 - What I Know.mp3` for reading
[2025-08-18T15:52:56Z DEBUG lofty::probe] Probe: Guessed file type `Some(Mpeg)` from extension
[2025-08-18T15:52:56Z DEBUG lofty::id3::v2::header] Parsing ID3v2 header
[2025-08-18T15:52:56Z DEBUG lofty::id3::v2::read] Parsing ID3v2 tag, size: 76901, version: V3
[2025-08-18T15:52:56Z DEBUG lofty::id3] Searching for an ID3v1 tag
[2025-08-18T15:52:56Z DEBUG lofty::id3] Found an ID3v1 tag, parsing
[2025-08-18T15:52:56Z DEBUG lofty::id3] Searching for a Lyrics3v2 tag
[2025-08-18T15:52:56Z WARN  lofty::mpeg::properties] MPEG: Using bitrate to estimate duration
[2025-08-18T15:52:56Z DEBUG lofty::mpeg::header] MPEG: Frame header uses a reserved layer
[2025-08-18T15:52:56Z DEBUG lofty::mpeg::properties] MPEG: VBR detected
[2025-08-18T15:52:56Z DEBUG lofty::probe] Probe: Opening `/Users/jstar/Downloads/ABB Tests/01 - Introduction/02 - Foreward.mp3` for reading
[2025-08-18T15:52:56Z DEBUG lofty::probe] Probe: Guessed file type `Some(Mpeg)` from extension
[2025-08-18T15:52:56Z DEBUG lofty::id3::v2::header] Parsing ID3v2 header
[2025-08-18T15:52:56Z DEBUG lofty::id3::v2::read] Parsing ID3v2 tag, size: 75146, version: V3
[2025-08-18T15:52:56Z DEBUG lofty::id3] Searching for an ID3v1 tag
[2025-08-18T15:52:56Z DEBUG lofty::id3] Found an ID3v1 tag, parsing
[2025-08-18T15:52:56Z DEBUG lofty::id3] Searching for a Lyrics3v2 tag
[2025-08-18T15:52:56Z WARN  lofty::mpeg::properties] MPEG: Using bitrate to estimate duration
[2025-08-18T15:52:56Z DEBUG lofty::mpeg::header] MPEG: Frame header uses a reserved layer
[2025-08-18T15:52:56Z DEBUG lofty::mpeg::header] MPEG: Frame header uses a reserved layer
[2025-08-18T15:52:56Z DEBUG lofty::mpeg::properties] MPEG: VBR detected
[2025-08-18T15:52:56Z DEBUG lofty::probe] Probe: Opening `/Users/jstar/Downloads/ABB Tests/01 - Introduction/03 - What I Do.mp3` for reading
[2025-08-18T15:52:56Z DEBUG lofty::probe] Probe: Guessed file type `Some(Mpeg)` from extension
[2025-08-18T15:52:56Z DEBUG lofty::id3::v2::header] Parsing ID3v2 header
[2025-08-18T15:52:56Z DEBUG lofty::id3::v2::read] Parsing ID3v2 tag, size: 76274, version: V3
[2025-08-18T15:52:56Z DEBUG lofty::id3] Searching for an ID3v1 tag
[2025-08-18T15:52:56Z DEBUG lofty::id3] Found an ID3v1 tag, parsing
[2025-08-18T15:52:56Z DEBUG lofty::id3] Searching for a Lyrics3v2 tag
[2025-08-18T15:52:56Z WARN  lofty::mpeg::properties] MPEG: Using bitrate to estimate duration
[2025-08-18T15:52:56Z DEBUG lofty::mpeg::properties] MPEG: VBR detected
[2025-08-18T15:52:56Z DEBUG lofty::probe] Probe: Opening `/Users/jstar/Downloads/ABB Tests/01 - Introduction/05 - The Unlived Life.mp3` for reading
[2025-08-18T15:52:56Z DEBUG lofty::probe] Probe: Guessed file type `Some(Mpeg)` from extension
[2025-08-18T15:52:56Z DEBUG lofty::id3::v2::header] Parsing ID3v2 header
[2025-08-18T15:52:56Z DEBUG lofty::id3::v2::read] Parsing ID3v2 tag, size: 75271, version: V3
[2025-08-18T15:52:56Z DEBUG lofty::id3] Searching for an ID3v1 tag
[2025-08-18T15:52:56Z DEBUG lofty::id3] Found an ID3v1 tag, parsing
[2025-08-18T15:52:56Z DEBUG lofty::id3] Searching for a Lyrics3v2 tag
[2025-08-18T15:52:56Z WARN  lofty::mpeg::properties] MPEG: Using bitrate to estimate duration
[2025-08-18T15:52:56Z DEBUG lofty::mpeg::properties] MPEG: VBR detected
[2025-08-18T15:52:56Z DEBUG lofty::probe] Probe: Opening `/Users/jstar/Downloads/ABB Tests/01 - Introduction/01 - Introduction.mp3` for reading
[2025-08-18T15:52:56Z DEBUG lofty::probe] Probe: Guessed file type `Some(Mpeg)` from extension
[2025-08-18T15:52:56Z DEBUG lofty::id3::v2::header] Parsing ID3v2 header
[2025-08-18T15:52:56Z DEBUG lofty::id3::v2::read] Parsing ID3v2 tag, size: 75021, version: V3
[2025-08-18T15:52:56Z DEBUG lofty::id3] Searching for an ID3v1 tag
[2025-08-18T15:52:56Z DEBUG lofty::id3] Found an ID3v1 tag, parsing
[2025-08-18T15:52:56Z DEBUG lofty::id3] Searching for a Lyrics3v2 tag
[2025-08-18T15:52:56Z WARN  lofty::mpeg::properties] MPEG: Using bitrate to estimate duration
[2025-08-18T15:52:56Z DEBUG lofty::mpeg::properties] MPEG: VBR detected
[2025-08-18T15:52:58Z DEBUG lofty::probe] Probe: Opening `/Users/jstar/Downloads/ABB Tests/01 - Introduction/01 - Introduction.mp3` for reading
[2025-08-18T15:52:58Z DEBUG lofty::probe] Probe: Guessed file type `Some(Mpeg)` from extension
[2025-08-18T15:52:58Z DEBUG lofty::id3::v2::header] Parsing ID3v2 header
[2025-08-18T15:52:58Z DEBUG lofty::id3::v2::read] Parsing ID3v2 tag, size: 75021, version: V3
[2025-08-18T15:52:58Z DEBUG lofty::id3] Searching for an ID3v1 tag
[2025-08-18T15:52:58Z DEBUG lofty::id3] Found an ID3v1 tag, parsing
[2025-08-18T15:52:58Z DEBUG lofty::id3] Searching for a Lyrics3v2 tag
[2025-08-18T15:52:58Z WARN  lofty::mpeg::properties] MPEG: Using bitrate to estimate duration
[2025-08-18T15:52:58Z DEBUG lofty::mpeg::properties] MPEG: VBR detected
[2025-08-18T15:53:07Z DEBUG lofty::probe] Probe: Opening `/Users/jstar/Downloads/ABB Tests/01 - Introduction/01 - Introduction.mp3` for reading
[2025-08-18T15:53:07Z DEBUG lofty::probe] Probe: Guessed file type `Some(Mpeg)` from extension
[2025-08-18T15:53:07Z DEBUG lofty::id3::v2::header] Parsing ID3v2 header
[2025-08-18T15:53:07Z DEBUG lofty::id3::v2::read] Parsing ID3v2 tag, size: 75021, version: V3
[2025-08-18T15:53:07Z DEBUG lofty::id3] Searching for an ID3v1 tag
[2025-08-18T15:53:07Z DEBUG lofty::id3] Found an ID3v1 tag, parsing
[2025-08-18T15:53:07Z DEBUG lofty::id3] Searching for a Lyrics3v2 tag
[2025-08-18T15:53:07Z WARN  lofty::mpeg::properties] MPEG: Using bitrate to estimate duration
[2025-08-18T15:53:07Z DEBUG lofty::mpeg::properties] MPEG: VBR detected
[2025-08-18T15:53:07Z DEBUG lofty::probe] Probe: Opening `/Users/jstar/Downloads/ABB Tests/01 - Introduction/01 - Introduction.mp3` for reading
[2025-08-18T15:53:07Z DEBUG lofty::probe] Probe: Guessed file type `Some(Mpeg)` from extension
[2025-08-18T15:53:07Z DEBUG lofty::id3::v2::header] Parsing ID3v2 header
[2025-08-18T15:53:07Z DEBUG lofty::id3::v2::read] Parsing ID3v2 tag, size: 75021, version: V3
[2025-08-18T15:53:07Z DEBUG lofty::id3] Searching for an ID3v1 tag
[2025-08-18T15:53:07Z DEBUG lofty::id3] Found an ID3v1 tag, parsing
[2025-08-18T15:53:07Z DEBUG lofty::id3] Searching for a Lyrics3v2 tag
[2025-08-18T15:53:07Z WARN  lofty::mpeg::properties] MPEG: Using bitrate to estimate duration
[2025-08-18T15:53:07Z DEBUG lofty::mpeg::properties] MPEG: VBR detected
[2025-08-18T15:53:07Z DEBUG lofty::probe] Probe: Opening `/Users/jstar/Downloads/ABB Tests/01 - Introduction/02 - Foreward.mp3` for reading
[2025-08-18T15:53:07Z DEBUG lofty::probe] Probe: Guessed file type `Some(Mpeg)` from extension
[2025-08-18T15:53:07Z DEBUG lofty::id3::v2::header] Parsing ID3v2 header
[2025-08-18T15:53:07Z DEBUG lofty::id3::v2::read] Parsing ID3v2 tag, size: 75146, version: V3
[2025-08-18T15:53:07Z DEBUG lofty::id3] Searching for an ID3v1 tag
[2025-08-18T15:53:07Z DEBUG lofty::id3] Found an ID3v1 tag, parsing
[2025-08-18T15:53:07Z DEBUG lofty::id3] Searching for a Lyrics3v2 tag
[2025-08-18T15:53:07Z WARN  lofty::mpeg::properties] MPEG: Using bitrate to estimate duration
[2025-08-18T15:53:07Z DEBUG lofty::mpeg::header] MPEG: Frame header uses a reserved layer
[2025-08-18T15:53:07Z DEBUG lofty::mpeg::header] MPEG: Frame header uses a reserved layer
[2025-08-18T15:53:07Z DEBUG lofty::mpeg::properties] MPEG: VBR detected
[2025-08-18T15:53:07Z DEBUG lofty::probe] Probe: Opening `/Users/jstar/Downloads/ABB Tests/01 - Introduction/03 - What I Do.mp3` for reading
[2025-08-18T15:53:07Z DEBUG lofty::probe] Probe: Guessed file type `Some(Mpeg)` from extension
[2025-08-18T15:53:07Z DEBUG lofty::id3::v2::header] Parsing ID3v2 header
[2025-08-18T15:53:07Z DEBUG lofty::id3::v2::read] Parsing ID3v2 tag, size: 76274, version: V3
[2025-08-18T15:53:07Z DEBUG lofty::id3] Searching for an ID3v1 tag
[2025-08-18T15:53:07Z DEBUG lofty::id3] Found an ID3v1 tag, parsing
[2025-08-18T15:53:07Z DEBUG lofty::id3] Searching for a Lyrics3v2 tag
[2025-08-18T15:53:07Z WARN  lofty::mpeg::properties] MPEG: Using bitrate to estimate duration
[2025-08-18T15:53:07Z DEBUG lofty::mpeg::properties] MPEG: VBR detected
[2025-08-18T15:53:07Z DEBUG lofty::probe] Probe: Opening `/Users/jstar/Downloads/ABB Tests/01 - Introduction/04 - What I Know.mp3` for reading
[2025-08-18T15:53:07Z DEBUG lofty::probe] Probe: Guessed file type `Some(Mpeg)` from extension
[2025-08-18T15:53:07Z DEBUG lofty::id3::v2::header] Parsing ID3v2 header
[2025-08-18T15:53:07Z DEBUG lofty::id3::v2::read] Parsing ID3v2 tag, size: 76901, version: V3
[2025-08-18T15:53:07Z DEBUG lofty::id3] Searching for an ID3v1 tag
[2025-08-18T15:53:07Z DEBUG lofty::id3] Found an ID3v1 tag, parsing
[2025-08-18T15:53:07Z DEBUG lofty::id3] Searching for a Lyrics3v2 tag
[2025-08-18T15:53:07Z WARN  lofty::mpeg::properties] MPEG: Using bitrate to estimate duration
[2025-08-18T15:53:07Z DEBUG lofty::mpeg::header] MPEG: Frame header uses a reserved layer
[2025-08-18T15:53:07Z DEBUG lofty::mpeg::properties] MPEG: VBR detected
[2025-08-18T15:53:07Z DEBUG lofty::probe] Probe: Opening `/Users/jstar/Downloads/ABB Tests/01 - Introduction/05 - The Unlived Life.mp3` for reading
[2025-08-18T15:53:07Z DEBUG lofty::probe] Probe: Guessed file type `Some(Mpeg)` from extension
[2025-08-18T15:53:07Z DEBUG lofty::id3::v2::header] Parsing ID3v2 header
[2025-08-18T15:53:07Z DEBUG lofty::id3::v2::read] Parsing ID3v2 tag, size: 75271, version: V3
[2025-08-18T15:53:07Z DEBUG lofty::id3] Searching for an ID3v1 tag
[2025-08-18T15:53:07Z DEBUG lofty::id3] Found an ID3v1 tag, parsing
[2025-08-18T15:53:07Z DEBUG lofty::id3] Searching for a Lyrics3v2 tag
[2025-08-18T15:53:07Z WARN  lofty::mpeg::properties] MPEG: Using bitrate to estimate duration
[2025-08-18T15:53:07Z DEBUG lofty::mpeg::properties] MPEG: VBR detected
[2025-08-18T15:53:07Z INFO  audiobook_boss_lib::audio::processor::execute] Starting FFmpeg merge - Total duration: 1024.50s, Bitrate: 64k
[2025-08-18T15:53:07Z DEBUG audiobook_boss_lib::audio::processor::execute] Using media processor: FfmpegNextProcessor (single-engine)
[mp3 @ 0x7919b6800] Estimating duration from bitrate, this may be inaccurate
[2025-08-18T15:53:07Z DEBUG audiobook_boss_lib::metadata::ffmpeg_bridge] Container metadata set via ffmpeg-next
[2025-08-18T15:53:07Z DEBUG audiobook_boss_lib::audio::media_pipeline] Container metadata set successfully
[2025-08-18T15:53:07Z DEBUG audiobook_boss_lib::audio::media_pipeline] Set strict=experimental on encoder context
[2025-08-18T15:53:07Z INFO  audiobook_boss_lib::audio::media_pipeline] AAC encoder configured for variable frame sizes
[2025-08-18T15:53:07Z WARN  audiobook_boss_lib::audio::media_pipeline] Twoloop AAC enhancement unavailable (Operation failed: Failed to set aac_coder option: FFmpeg error code -1414549496), falling back to standard AAC-LC
[2025-08-18T15:53:07Z INFO  audiobook_boss_lib::audio::media_pipeline] cover_art_plan decision=native_attempt bytes=74732
[2025-08-18T15:53:07Z INFO  audiobook_boss_lib::audio::media_pipeline] Attempting native cover art embedding - 74732 bytes of cover data
[2025-08-18T15:53:07Z DEBUG audiobook_boss_lib::metadata::ffmpeg_bridge] Configured cover art stream parameters for Jpeg format (680x680)
[2025-08-18T15:53:07Z DEBUG audiobook_boss_lib::metadata::ffmpeg_bridge] Set ATTACHED_PIC disposition on stream 1
[2025-08-18T15:53:07Z INFO  audiobook_boss_lib::metadata::ffmpeg_bridge] Added cover art stream with attached_pic disposition (index=1, format=Jpeg, bytes=74732)
[2025-08-18T15:53:07Z INFO  audiobook_boss_lib::audio::media_pipeline] ✓ Native cover art stream added successfully (stream=1, format=Jpeg) - will embed during encoding
[2025-08-18T15:53:07Z INFO  audiobook_boss_lib::audio::media_pipeline] Writing cover art packet to stream 1 (Jpeg format, 74732 bytes)
[2025-08-18T15:53:07Z INFO  audiobook_boss_lib::metadata::ffmpeg_bridge] Cover art packet written as attached pic (stream=1, format=Jpeg, size=74732 bytes)
[2025-08-18T15:53:07Z INFO  audiobook_boss_lib::audio::media_pipeline] ✓ Native cover art packet written successfully to stream 1
[2025-08-18T15:53:07Z DEBUG audiobook_boss_lib::metadata::ffmpeg_bridge] Cover art format validation: JPEG detected and supported
[2025-08-18T15:53:07Z DEBUG audiobook_boss_lib::audio::cleanup::guard] Creating cleanup guard for session: ddfd8727-8a8f-49c4-8b8c-8f3699b6d552
[2025-08-18T15:53:07Z DEBUG audiobook_boss_lib::audio::cleanup::guard] Session ddfd8727-8a8f-49c4-8b8c-8f3699b6d552: Adding path to cleanup: /var/folders/29/g7wmd39x0pb75l_n1hjrttcw0000gn/T/audiobook-boss/ddfd8727-8a8f-49c4-8b8c-8f3699b6d552/merged.m4b
[2025-08-18T15:53:07Z INFO  audiobook_boss_lib::audio::media_pipeline] Starting audio processing for 5 input files
[2025-08-18T15:53:07Z INFO  audiobook_boss_lib::audio::media_pipeline] Processing input file 1/5: /Users/jstar/Downloads/ABB Tests/01 - Introduction/01 - Introduction.mp3
[2025-08-18T15:53:07Z INFO  audiobook_boss_lib::audio::media_pipeline] 🎵 Starting to process input file: /Users/jstar/Downloads/ABB Tests/01 - Introduction/01 - Introduction.mp3
[2025-08-18T15:53:07Z INFO  audiobook_boss_lib::audio::media_pipeline] Setting up decoder and resampler for: /Users/jstar/Downloads/ABB Tests/01 - Introduction/01 - Introduction.mp3
[2025-08-18T15:53:07Z INFO  audiobook_boss_lib::audio::media_pipeline] 🔧 Setting up decoder for input file: /Users/jstar/Downloads/ABB Tests/01 - Introduction/01 - Introduction.mp3
[2025-08-18T15:53:07Z INFO  audiobook_boss_lib::audio::media_pipeline] ✓ Input file exists and is accessible
[2025-08-18T15:53:07Z INFO  audiobook_boss_lib::audio::media_pipeline] Opening FFmpeg input context...
[mp3 @ 0x7920a0280] Estimating duration from bitrate, this may be inaccurate
[2025-08-18T15:53:07Z INFO  audiobook_boss_lib::audio::media_pipeline] ✓ FFmpeg input context opened successfully
[2025-08-18T15:53:07Z INFO  audiobook_boss_lib::audio::media_pipeline] Finding best audio stream...
[2025-08-18T15:53:07Z INFO  audiobook_boss_lib::audio::media_pipeline] ✓ Found audio stream at index: 0
[2025-08-18T15:53:07Z INFO  audiobook_boss_lib::audio::media_pipeline] Creating decoder context from stream parameters...
[2025-08-18T15:53:07Z INFO  audiobook_boss_lib::audio::media_pipeline] ✓ Decoder context created successfully
[2025-08-18T15:53:07Z INFO  audiobook_boss_lib::audio::media_pipeline] Opening audio decoder...
[2025-08-18T15:53:07Z INFO  audiobook_boss_lib::audio::media_pipeline] ✓ Audio decoder opened successfully
[2025-08-18T15:53:07Z INFO  audiobook_boss_lib::audio::media_pipeline] Creating resampler...
[2025-08-18T15:53:07Z INFO  audiobook_boss_lib::audio::media_pipeline] Input audio format: rate=44100, channels=ChannelLayout { is_empty: false, channels: 2, u.mask: 3 }, format=F32(Planar)
[2025-08-18T15:53:07Z INFO  audiobook_boss_lib::audio::media_pipeline] Output audio format: rate=44100, channels=ChannelLayout { is_empty: false, channels: 2, u.mask: 3 }, format=F32(Planar)
[2025-08-18T15:53:07Z INFO  audiobook_boss_lib::audio::media_pipeline] ✓ Resampler created successfully
[2025-08-18T15:53:07Z INFO  audiobook_boss_lib::audio::media_pipeline] 🎉 Decoder and resampler setup completed for: /Users/jstar/Downloads/ABB Tests/01 - Introduction/01 - Introduction.mp3
[2025-08-18T15:53:07Z INFO  audiobook_boss_lib::audio::media_pipeline] ✓ Decoder and resampler setup complete for stream index: 0
[2025-08-18T15:53:07Z INFO  audiobook_boss_lib::audio::media_pipeline] Updated context: file_index=0, stream_index=0
[2025-08-18T15:53:07Z INFO  audiobook_boss_lib::audio::media_pipeline] Processing input packets from: /Users/jstar/Downloads/ABB Tests/01 - Introduction/01 - Introduction.mp3
[2025-08-18T15:53:07Z INFO  audiobook_boss_lib::audio::media_pipeline] 📦 Starting packet processing for stream index: 0
[2025-08-18T15:53:07Z INFO  audiobook_boss_lib::audio::media_pipeline] Starting packet iteration...
[2025-08-18T15:53:07Z DEBUG audiobook_boss_lib::audio::media_pipeline] Processing packet from stream 1
[2025-08-18T15:53:07Z DEBUG audiobook_boss_lib::audio::media_pipeline] Skipping packet from stream 1 (expecting 0)
[2025-08-18T15:53:07Z DEBUG audiobook_boss_lib::audio::media_pipeline] Processing packet from stream 0
[2025-08-18T15:53:07Z DEBUG audiobook_boss_lib::audio::media_pipeline] Sending packet 1 to decoder
[2025-08-18T15:53:07Z DEBUG audiobook_boss_lib::audio::media_pipeline] ✓ Packet 1 sent to decoder successfully
[2025-08-18T15:53:07Z DEBUG audiobook_boss_lib::audio::media_pipeline] Processing decoded frames for packet 1
[2025-08-18T15:53:07Z DEBUG audiobook_boss_lib::audio::media_pipeline] ✓ Decoded frames processed successfully for packet 1
[2025-08-18T15:53:07Z DEBUG audiobook_boss_lib::audio::media_pipeline] Processing packet from stream 0
[2025-08-18T15:53:07Z DEBUG audiobook_boss_lib::audio::media_pipeline] Sending packet 2 to decoder
[2025-08-18T15:53:07Z DEBUG audiobook_boss_lib::audio::media_pipeline] ✓ Packet 2 sent to decoder successfully
[2025-08-18T15:53:07Z DEBUG audiobook_boss_lib::audio::media_pipeline] Processing decoded frames for packet 2
[2025-08-18T15:53:07Z DEBUG audiobook_boss_lib::audio::media_pipeline] ✓ Decoded frames processed successfully for packet 2
[2025-08-18T15:53:07Z DEBUG audiobook_boss_lib::audio::media_pipeline] Processing packet from stream 0
[2025-08-18T15:53:07Z DEBUG audiobook_boss_lib::audio::media_pipeline] Sending packet 3 to decoder
[2025-08-18T15:53:07Z DEBUG audiobook_boss_lib::audio::media_pipeline] ✓ Packet 3 sent to decoder successfully
[2025-08-18T15:53:07Z DEBUG audiobook_boss_lib::audio::media_pipeline] Processing decoded frames for packet 3
[2025-08-18T15:53:07Z DEBUG audiobook_boss_lib::audio::media_pipeline] ✓ Decoded frames processed successfully for packet 3
[2025-08-18T15:53:07Z DEBUG audiobook_boss_lib::audio::media_pipeline] Processing packet from stream 0
[2025-08-18T15:53:07Z DEBUG audiobook_boss_lib::audio::media_pipeline] Sending packet 4 to decoder
[2025-08-18T15:53:07Z DEBUG audiobook_boss_lib::audio::media_pipeline] ✓ Packet 4 sent to decoder successfully
[2025-08-18T15:53:07Z DEBUG audiobook_boss_lib::audio::media_pipeline] Processing decoded frames for packet 4
[2025-08-18T15:53:07Z DEBUG audiobook_boss_lib::audio::media_pipeline] ✓ Decoded frames processed successfully for packet 4
[2025-08-18T15:53:07Z DEBUG audiobook_boss_lib::audio::media_pipeline] Processing packet from stream 0
[2025-08-18T15:53:07Z DEBUG audiobook_boss_lib::audio::media_pipeline] Sending packet 5 to decoder
[2025-08-18T15:53:07Z DEBUG audiobook_boss_lib::audio::media_pipeline] ✓ Packet 5 sent to decoder successfully
[2025-08-18T15:53:07Z DEBUG audiobook_boss_lib::audio::media_pipeline] Processing decoded frames for packet 5
[2025-08-18T15:53:07Z DEBUG audiobook_boss_lib::audio::media_pipeline] ✓ Decoded frames processed successfully for packet 5
[2025-08-18T15:53:07Z DEBUG audiobook_boss_lib::audio::media_pipeline] Processing packet from stream 0
[2025-08-18T15:53:07Z DEBUG audiobook_boss_lib::audio::media_pipeline] Sending packet 6 to decoder
[2025-08-18T15:53:07Z DEBUG audiobook_boss_lib::audio::media_pipeline] ✓ Packet 6 sent to decoder successfully
[2025-08-18T15:53:07Z DEBUG audiobook_boss_lib::audio::media_pipeline] Processing decoded frames for packet 6
[2025-08-18T15:53:07Z DEBUG audiobook_boss_lib::audio::media_pipeline] ✓ Decoded frames processed successfully for packet 6
[2025-08-18T15:53:07Z DEBUG audiobook_boss_lib::audio::media_pipeline] Processing packet from stream 0
[2025-08-18T15:53:07Z DEBUG audiobook_boss_lib::audio::media_pipeline] Sending packet 7 to decoder
[2025-08-18T15:53:07Z DEBUG audiobook_boss_lib::audio::media_pipeline] ✓ Packet 7 sent to decoder successfully
[2025-08-18T15:53:07Z DEBUG audiobook_boss_lib::audio::media_pipeline] Processing decoded frames for packet 7
[2025-08-18T15:53:07Z DEBUG audiobook_boss_lib::audio::media_pipeline] ✓ Decoded frames processed successfully for packet 7
[2025-08-18T15:53:07Z DEBUG audiobook_boss_lib::audio::media_pipeline] Processing packet from stream 0
[2025-08-18T15:53:07Z DEBUG audiobook_boss_lib::audio::media_pipeline] Sending packet 8 to decoder
[2025-08-18T15:53:07Z DEBUG audiobook_boss_lib::audio::media_pipeline] ✓ Packet 8 sent to decoder successfully
[2025-08-18T15:53:07Z DEBUG audiobook_boss_lib::audio::media_pipeline] Processing decoded frames for packet 8
[2025-08-18T15:53:07Z DEBUG audiobook_boss_lib::audio::media_pipeline] ✓ Decoded frames processed successfully for packet 8
[2025-08-18T15:53:07Z DEBUG audiobook_boss_lib::audio::media_pipeline] Processing packet from stream 0
[2025-08-18T15:53:07Z DEBUG audiobook_boss_lib::audio::media_pipeline] Sending packet 9 to decoder
[2025-08-18T15:53:07Z DEBUG audiobook_boss_lib::audio::media_pipeline] ✓ Packet 9 sent to decoder successfully
[2025-08-18T15:53:07Z DEBUG audiobook_boss_lib::audio::media_pipeline] Processing decoded frames for packet 9
[2025-08-18T15:53:07Z DEBUG audiobook_boss_lib::audio::media_pipeline] ✓ Decoded frames processed successfully for packet 9
[2025-08-18T15:53:07Z DEBUG audiobook_boss_lib::audio::media_pipeline] Processing packet from stream 0
[2025-08-18T15:53:07Z DEBUG audiobook_boss_lib::audio::media_pipeline] Sending packet 10 to decoder
[2025-08-18T15:53:07Z DEBUG audiobook_boss_lib::audio::media_pipeline] ✓ Packet 10 sent to decoder successfully
[2025-08-18T15:53:07Z DEBUG audiobook_boss_lib::audio::media_pipeline] Processing decoded frames for packet 10
[aac @ 0x78ecf5500] Input contains (near) NaN/+-Inf
[2025-08-18T15:53:07Z ERROR audiobook_boss_lib::audio::media_pipeline] ✗ Failed to process decoded frames for packet 10: Operation failed: Encoder send failed: Invalid argument
[2025-08-18T15:53:07Z DEBUG audiobook_boss_lib::audio::cleanup::guard] Session ddfd8727-8a8f-49c4-8b8c-8f3699b6d552: Cleaning up 1 paths on drop
[2025-08-18T15:53:07Z DEBUG audiobook_boss_lib::audio::cleanup::ops] Session ddfd8727-8a8f-49c4-8b8c-8f3699b6d552: Removing file: /var/folders/29/g7wmd39x0pb75l_n1hjrttcw0000gn/T/audiobook-boss/ddfd8727-8a8f-49c4-8b8c-8f3699b6d552/merged.m4b
[2025-08-18T15:53:07Z DEBUG audiobook_boss_lib::audio::cleanup::ops] Session ddfd8727-8a8f-49c4-8b8c-8f3699b6d552: All cleanup operations completed successfully
[aac @ 0x78ecf5500] Qavg: 26819.818
[aac @ 0x78ecf5500] 3 frames left in the queue on closing