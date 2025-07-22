import { invoke } from "@tauri-apps/api/core";
import type { AudiobookMetadata } from "./types/metadata";
import type { FileListInfo, AudioSettings } from "./types/audio";

// Expose test functions for console access
(window as any).testCommands = {
  ping: () => invoke('ping'),
  echo: (input: string) => invoke('echo', { input }),
  validateFiles: (paths: string[]) => invoke('validate_files', { file_paths: paths }),
  getFFmpegVersion: () => invoke('get_ffmpeg_version'),
  mergeAudioFiles: (file1: string, file2: string) => invoke('merge_audio_files', { file1, file2 }),
  
  // Metadata commands
  readMetadata: (filePath: string) => invoke<AudiobookMetadata>('read_audio_metadata', { filePath }),
  writeMetadata: (filePath: string, metadata: AudiobookMetadata) => 
    invoke('write_audio_metadata', { filePath, metadata }),
  writeCoverArt: (filePath: string, coverData: number[]) => 
    invoke('write_cover_art', { filePath, coverData }),
  
  // Audio processing commands
  analyzeAudioFiles: (filePaths: string[]) => invoke<FileListInfo>('analyze_audio_files', { filePaths }),
  validateAudioSettings: (settings: AudioSettings) => invoke('validate_audio_settings', { settings }),
  processAudiobook: (filePaths: string[], settings: AudioSettings, metadata?: AudiobookMetadata) => 
    invoke('process_audiobook_files', { filePaths, settings, metadata })
};

// Log when ready
console.log('Test commands available:');
console.log('  window.testCommands.ping()');
console.log('  window.testCommands.echo(input)');
console.log('  window.testCommands.validateFiles(paths)');
console.log('  window.testCommands.getFFmpegVersion()');
console.log('  window.testCommands.mergeAudioFiles(file1, file2)');
console.log('  window.testCommands.readMetadata(filePath)');
console.log('  window.testCommands.writeMetadata(filePath, metadata)');
console.log('  window.testCommands.writeCoverArt(filePath, coverData)');
console.log('  window.testCommands.analyzeAudioFiles(filePaths)');
console.log('  window.testCommands.validateAudioSettings(settings)');
console.log('  window.testCommands.processAudiobook(filePaths, settings, metadata?)');
