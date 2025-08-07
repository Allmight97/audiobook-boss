🔍 AUDIOBOOK-BOSS CODE ANALYSIS
===============================

📊 MODULE LINE COUNTS (Source Files Only)
----------------------------------------
    7789 total
     631 ./src-tauri/src/audio/processor.rs
     521 ./src/ui/fileList.ts
     485 ./src-tauri/src/audio/progress.rs
     480 ./src-tauri/src/audio/cleanup.rs
     443 ./src/ui/statusPanel.ts
     438 ./src-tauri/src/commands/mod.rs
     422 ./src-tauri/tests/audio/processor_tests.rs
     411 ./src-tauri/src/tests_integration.rs
     391 ./src-tauri/src/audio/file_list.rs
     362 ./src-tauri/src/audio/progress_monitor.rs
     361 ./src/ui/outputPanel.ts
     328 ./src-tauri/src/audio/context.rs
     242 ./src-tauri/src/ffmpeg/command.rs
     205 ./src/ui/coverArt.ts
     199 ./src-tauri/src/audio/settings.rs
     196 ./src/types/events.ts
     183 ./src-tauri/src/audio/media_pipeline.rs
     169 ./src-tauri/src/audio/constants.rs
     153 ./src-tauri/src/audio/mod.rs
     141 ./src-tauri/src/audio/metrics.rs
     133 ./src-tauri/src/metadata/writer.rs
     112 ./src/main.ts
     108 ./src/ui/fileImport.ts
     106 ./src/types/audio.ts
     100 ./src-tauri/src/audio/session.rs
      97 ./src-tauri/src/ffmpeg/mod.rs
      76 ./src-tauri/src/metadata/reader.rs
      68 ./src-tauri/src/errors.rs
      57 ./src-tauri/src/lib.rs
      56 ./src-tauri/src/metadata/mod.rs
      55 ./src-tauri/tests/unit/audio/cleanup_tests.rs
      54 ./src/types/metadata.ts
       6 ./src-tauri/src/main.rs

📈 SUMMARY BY LANGUAGE
---------------------
Rust files:     5206 lines
TypeScript:     2106 lines
Total:          7312 lines

🎯 FILES WITH POTENTIAL LARGE FUNCTIONS
=======================================

📦 FILES >300 LINES (likely contain large functions)
---------------------------------------------------
./src-tauri/src/audio/processor.rs                  631 lines
./src/ui/fileList.ts                                521 lines
./src-tauri/src/audio/progress.rs                   485 lines
./src-tauri/src/audio/cleanup.rs                    480 lines
./src/ui/statusPanel.ts                             443 lines
./src-tauri/src/commands/mod.rs                     438 lines
./src-tauri/tests/audio/processor_tests.rs          422 lines
./src-tauri/src/tests_integration.rs                411 lines
./src-tauri/src/audio/file_list.rs                  391 lines
./src-tauri/src/audio/progress_monitor.rs           362 lines
./src/ui/outputPanel.ts                             361 lines
./src-tauri/src/audio/context.rs                    328 lines

🔍 QUICK LARGE FUNCTION CHECK
-----------------------------
Rust functions with >50 lines of implementation:
./src-tauri/src/ffmpeg/mod.rs                      locate_ffmpeg        ~61 lines
./src-tauri/src/tests_integration.rs               test_current_audio_processing_flow ~58 lines
./src-tauri/src/tests_integration.rs               test_file_validation ~72 lines
./src-tauri/src/audio/processor.rs                 process_audiobook    ~72 lines
./src-tauri/src/audio/processor.rs                 merge_audio_files_with_events ~56 lines
./src-tauri/src/audio/cleanup.rs                   drop                 ~59 lines
./src-tauri/src/audio/file_list.rs                 test_debug_real_mp3_file ~67 lines
./src-tauri/src/audio/file_list.rs                 test_debug_lofty_m4b_errors ~84 lines
./src-tauri/src/audio/mod.rs                       new                  ~51 lines
./src-tauri/src/audio/mod.rs                       default              ~55 lines

TypeScript functions with >50 lines of implementation:
./src/ui/statusPanel.ts                            getElementValue      ~70 lines
./src/ui/fileList.ts                               updateFileListDOM    ~51 lines

📋 LARGEST MODULES (TOP 10)
===========================
     631 ./src-tauri/src/audio/processor.rs
     521 ./src/ui/fileList.ts
     485 ./src-tauri/src/audio/progress.rs
     480 ./src-tauri/src/audio/cleanup.rs
     443 ./src/ui/statusPanel.ts
     438 ./src-tauri/src/commands/mod.rs
     422 ./src-tauri/tests/audio/processor_tests.rs
     411 ./src-tauri/src/tests_integration.rs
     391 ./src-tauri/src/audio/file_list.rs
     362 ./src-tauri/src/audio/progress_monitor.rs

💡 RECOMMENDATIONS:
- Files >400 lines should be considered for splitting
- Functions >60 lines often benefit from refactoring
- Look for repeated patterns that can be extracted
