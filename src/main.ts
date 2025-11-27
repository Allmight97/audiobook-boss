import { bridge } from "./lib/bridge";
import type { AudiobookMetadata } from "./types/metadata";
import type { FileListInfo } from "./types/audio";
import { initFileImport } from "./ui/fileImport";
import { displayFileList, currentFileList, clearAllFiles, toggleFileSort, moveFileUp, moveFileDown } from "./ui/fileList";
import { initOutputPanel, getCurrentAudioSettings, onFileListChange, onMetadataChange } from "./ui/outputPanel";
import { initStatusPanel, getStatusPanel } from "./ui/statusPanel";
import { initEncoderPanel } from "./ui/encoderPanel";
import { initCoverArt, getCurrentCoverArt, setCoverArt, clearCoverArt } from "./ui/coverArt";
import { initTagPreview, updateTagPreview } from "./ui/tagPreview";

// Expose test functions for console access
(window as any).testCommands = {
  ping: () => bridge.invoke('ping'),
  echo: (input: string) => bridge.invoke('echo', { input }),
  validateFiles: (paths: string[]) => bridge.invoke('validate_files', { filePaths: paths }),
  // Removed getFFmpegVersion and mergeAudioFiles test commands after nuclear cleanup

  // Metadata commands
  readMetadata: (filePath: string) => bridge.invoke<AudiobookMetadata>('read_audio_metadata', { filePath: filePath }),
  writeMetadata: (filePath: string, metadata: AudiobookMetadata) =>
    bridge.invoke('write_audio_metadata', { filePath: filePath, metadata }),
  writeCoverArt: (filePath: string, coverData: number[]) =>
    bridge.invoke('write_cover_art', { filePath: filePath, coverData: coverData }),
  loadCoverArtFile: (filePath: string) => bridge.invoke('load_cover_art_file', { filePath }),

  // Audio processing commands
  analyzeAudioFiles: (filePaths: string[]) => bridge.invoke<FileListInfo>('analyze_audio_files', { filePaths: filePaths }),
  // UI test functions
  testDisplayList: (fileListInfo: FileListInfo) => displayFileList(fileListInfo),
  getCurrentFileList: () => currentFileList,
  clearFiles: () => clearAllFiles(),
  toggleSort: () => toggleFileSort(),
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
  cancelProcessing: () => bridge.invoke('cancel_processing'),

  // Cover art test functions
  getCurrentCoverArt: () => getCurrentCoverArt(),
  setCoverArt: (coverArtBytes: number[] | null) => setCoverArt(coverArtBytes),
  clearCoverArt: () => clearCoverArt(),

  // File movement test functions
  testMoveFile: (index: number, direction: 'up' | 'down') => {
    if (direction === 'up') {
      moveFileUp(index);
    } else if (direction === 'down') {
      moveFileDown(index);
    }
  },
  testSortFiles: () => toggleFileSort(),

  // Tag preview test functions
  updateTagPreview: () => updateTagPreview(),
};

// Log when ready
console.log('Test commands available:');
console.log('  window.testCommands.ping()');
console.log('  window.testCommands.echo(input)');
console.log('  window.testCommands.validateFiles(paths)');
// Removed: getFFmpegVersion, mergeAudioFiles
console.log('  window.testCommands.readMetadata(filePath)');
console.log('  window.testCommands.writeMetadata(filePath, metadata)');
console.log('  window.testCommands.writeCoverArt(filePath, coverData)');
console.log('  window.testCommands.analyzeAudioFiles(filePaths)');
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
console.log('  window.testCommands.testMoveFile(index, direction)');
console.log('  window.testCommands.testSortFiles()');
console.log('  window.testCommands.updateTagPreview()');

// Initialize UI components when DOM is ready
document.addEventListener('DOMContentLoaded', () => {
  initFileImport();
  initOutputPanel();
  initStatusPanel();
  initCoverArt();
  // Initialize Advanced Encoder panel (no-op if panel not present)
  initEncoderPanel();
  // Initialize tag preview grid
  initTagPreview();
  console.log('File import system initialized');
  console.log('Output panel initialized');
  console.log('Status panel initialized');
  console.log('Cover art system initialized');
  console.log('Tag preview initialized');
});
