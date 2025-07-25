import { invoke } from "@tauri-apps/api/core";
import type { AudiobookMetadata } from "./types/metadata";
import type { FileListInfo, AudioSettings } from "./types/audio";
import { initFileImport } from "./ui/fileImport";
import { displayFileList, currentFileList, clearAllFiles } from "./ui/fileList";
import { initOutputPanel, getCurrentAudioSettings, onFileListChange, onMetadataChange } from "./ui/outputPanel";
import { initStatusPanel, getStatusPanel } from "./ui/statusPanel";
import { initCoverArt, getCurrentCoverArt, setCoverArt, clearCoverArt } from "./ui/coverArt";

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
  loadCoverArtFile: (filePath: string) => invoke('load_cover_art_file', { filePath }),
  
  // Audio processing commands
  analyzeAudioFiles: (filePaths: string[]) => invoke<FileListInfo>('analyze_audio_files', { filePaths: filePaths }),
  validateAudioSettings: (settings: AudioSettings) => invoke('validate_audio_settings', { settings }),
  processAudiobook: (filePaths: string[], settings: AudioSettings, metadata?: AudiobookMetadata) => 
    invoke('process_audiobook_files', { filePaths: filePaths, settings, metadata }),

  // UI test functions
  testDisplayList: (fileListInfo: FileListInfo) => displayFileList(fileListInfo),
  getCurrentFileList: () => currentFileList,
  clearFiles: () => clearAllFiles(),
  // Test art thumbnail functionality
  testArtThumbnail: async () => {
    const statusPanel = getStatusPanel();
    if (statusPanel) {
      console.log('Testing art thumbnail update...');
      await (statusPanel as any).updateArtThumbnail();
      return 'Art thumbnail test completed - check the progress panel';
    }
    return 'StatusPanel not initialized';
  },
  
  // Output panel test functions
  getCurrentAudioSettings: () => getCurrentAudioSettings(),
  triggerFileListChange: () => onFileListChange(),
  triggerMetadataChange: () => onMetadataChange(),
  
  // Status panel test functions
  cancelProcessing: () => invoke('cancel_processing'),
  
  // Cover art test functions
  getCurrentCoverArt: () => getCurrentCoverArt(),
  setCoverArt: (coverArtBytes: number[] | null) => setCoverArt(coverArtBytes),
  clearCoverArt: () => clearCoverArt()
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
console.log('  window.testCommands.testArtThumbnail()');
console.log('  window.testCommands.loadCoverArtFile(filePath)');
console.log('  window.testCommands.getCurrentCoverArt()');
console.log('  window.testCommands.setCoverArt(coverArtBytes)');
console.log('  window.testCommands.clearCoverArt()');

// Initialize UI components when DOM is ready
document.addEventListener('DOMContentLoaded', () => {
  initFileImport();
  initOutputPanel();
  initStatusPanel();
  initCoverArt();
  console.log('File import system initialized');
  console.log('Output panel initialized');
  console.log('Status panel initialized');
  console.log('Cover art system initialized');
});
