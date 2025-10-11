## External APIs index → code surfaces

This index connects API docs to the primary code locations where they are applied.

### ffmpeg-next.md
- Code: `src-tauri/src/audio/processor/{encoder.rs,frame_pipeline.rs,streams.rs}`
- Support: `src-tauri/src/audio/{buffer.rs,frame_accumulator.rs}`
- Metadata mapping: `src-tauri/src/metadata/ffmpeg_bridge.rs`

### lofty.md
- Code: `src-tauri/src/metadata/{reader.rs,writer.rs}`
- Fallback checks: `src-tauri/src/audio/processor/finalize.rs`
- Analysis: `src-tauri/src/audio/file_list.rs`

### tauri-patterns.md
- Emit: `src-tauri/src/audio/progress/reporter.rs`
- Types: `src/types/events.ts`
- UI listeners: `src/ui/statusPanel`
- Invocations: `src/main.ts`

### tauri-commands.md
- Command surfaces: `src-tauri/src/commands/*`
- UI integrations: `src/ui/{fileImport,statusPanel,coverArt}`
- QA harness: `src/main.ts::window.testCommands`

### tauri-ts-boundaries.md
- Types: `src/types/events.ts`
- Frontend invocations: `src/main.ts`
- Rust commands: `src-tauri/src/commands/*`
- UI consumption: `src/ui/statusPanel`

### path-handling.md
- Validation: `src-tauri/src/audio/path_validation.rs`
- Analysis: `src-tauri/src/audio/file_list.rs`
- Revalidation before processing: `src-tauri/src/audio/processor/prepare.rs`


