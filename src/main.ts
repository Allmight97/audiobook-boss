import { invoke } from "@tauri-apps/api/core";

// Expose test functions for console access
(window as any).testCommands = {
  ping: () => invoke('ping'),
  echo: (input: string) => invoke('echo', { input }),
  validateFiles: (paths: string[]) => invoke('validate_files', { file_paths: paths }),
  getFFmpegVersion: () => invoke('get_ffmpeg_version'),
  mergeAudioFiles: (file1: string, file2: string) => invoke('merge_audio_files', { file1, file2 })
};

// Log when ready
console.log('Test commands available:');
console.log('  window.testCommands.ping()');
console.log('  window.testCommands.echo(input)');
console.log('  window.testCommands.validateFiles(paths)');
console.log('  window.testCommands.getFFmpegVersion()');
console.log('  window.testCommands.mergeAudioFiles(file1, file2)');
