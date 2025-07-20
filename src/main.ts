import { invoke } from "@tauri-apps/api/core";

// Expose test functions for console access
(window as any).testCommands = {
  ping: () => invoke('ping'),
  echo: (input: string) => invoke('echo', { input }),
  validateFiles: (paths: string[]) => invoke('validate_files', { file_paths: paths })
};

// Log when ready
console.log('Test commands available: window.testCommands.ping(), .echo(input), .validateFiles(paths)');
