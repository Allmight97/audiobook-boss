import { invoke } from "@tauri-apps/api/core";
import type { AudiobookMetadata } from "./types/metadata";
import type { FileListInfo, AudioSettings } from "./types/audio";
import { initFileImport } from "./ui/fileImport";
import { displayFileList, currentFileList, clearAllFiles } from "./ui/fileList";
import { initOutputPanel, getCurrentAudioSettings, onFileListChange, onMetadataChange } from "./ui/outputPanel";

// Expose test functions for console access
(window as any).testCommands = {
  ping: () => invoke('ping'),
  echo: (input: string) => invoke('echo', { input }),
  validateFiles: (paths: string[]) => invoke('validate_files', { filePaths: paths }),
  getFFmpegVersion: () => invoke('get_ffmpeg_version'),
  mergeAudioFiles: (file1: string, file2: string) => invoke('merge_audio_files', { file1, file2 }),
  
  // Metadata commands
  readMetadata: (filePath: string) => invoke<AudiobookMetadata>('read_audio_metadata', { filePath: filePath }),
  writeMetadata: (filePath: string, metadata: AudiobookMetadata) => 
    invoke('write_audio_metadata', { filePath: filePath, metadata }),
  writeCoverArt: (filePath: string, coverData: number[]) => 
    invoke('write_cover_art', { filePath: filePath, coverData: coverData }),
  
  // Audio processing commands
  analyzeAudioFiles: (filePaths: string[]) => invoke<FileListInfo>('analyze_audio_files', { filePaths: filePaths }),
  validateAudioSettings: (settings: AudioSettings) => invoke('validate_audio_settings', { settings }),
  processAudiobook: (filePaths: string[], settings: AudioSettings, metadata?: AudiobookMetadata) => 
    invoke('process_audiobook_files', { filePaths: filePaths, settings, metadata }),

  // UI test functions
  testDisplayList: (fileListInfo: FileListInfo) => displayFileList(fileListInfo),
  getCurrentFileList: () => currentFileList,
  clearFiles: () => clearAllFiles(),
  
  // Output panel test functions
  getCurrentAudioSettings: () => getCurrentAudioSettings(),
  triggerFileListChange: () => onFileListChange(),
  triggerMetadataChange: () => onMetadataChange()
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
console.log('  window.testCommands.testDisplayList(fileListInfo)');
console.log('  window.testCommands.getCurrentFileList()');
console.log('  window.testCommands.clearFiles()');
console.log('  window.testCommands.getCurrentAudioSettings()');
console.log('  window.testCommands.triggerFileListChange()');
console.log('  window.testCommands.triggerMetadataChange()');

// Initialize UI components when DOM is ready
document.addEventListener('DOMContentLoaded', () => {
  initFileImport();
  initOutputPanel();
  console.log('File import system initialized');
  console.log('Output panel initialized');
});
