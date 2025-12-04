import { bridge } from "./lib/bridge";
import type { AudiobookMetadata } from "./types/metadata";
import type { FileListInfo, EncoderSettings } from "./types/audio";
import { initFileImport } from "./ui/fileImport";
import {
  displayFileList,
  currentFileList,
  selectedFileIndex,
  clearAllFiles,
  toggleFileSort,
  moveFileUp,
  moveFileDown,
} from "./ui/fileList";
import {
  initOutputPanel,
  getCurrentAudioSettings,
  onFileListChange,
  onMetadataChange,
} from "./ui/outputPanel";
import { initStatusPanel, getStatusPanel } from "./ui/statusPanel";
import { initEncoderPanel } from "./ui/encoderPanel";
import {
  initCoverArt,
  getCurrentCoverArt,
  setCoverArt,
  clearCoverArt,
} from "./ui/coverArt";
import { initTagPreview, updateTagPreview } from "./ui/tagPreview";

// Expose test functions for console access
(window as any).testCommands = {
  ping: () => bridge.invoke("ping"),
  echo: (input: string) => bridge.invoke("echo", { input }),
  validateFiles: (paths: string[]) =>
    bridge.invoke("validate_files", { filePaths: paths }),
  // Removed getFFmpegVersion and mergeAudioFiles test commands after nuclear cleanup

  // Metadata commands
  readMetadata: (filePath: string) =>
    bridge.invoke<AudiobookMetadata>("read_audio_metadata", {
      filePath: filePath,
    }),
  validateEncoderSettings: (settings: EncoderSettings) =>
    bridge.invoke("validate_encoder_settings_cmd", settings),
  writeMetadata: (filePath: string, metadata: AudiobookMetadata) =>
    bridge.invoke("write_audio_metadata", { filePath: filePath, metadata }),
  writeCoverArt: (filePath: string, coverData: number[]) =>
    bridge.invoke("write_cover_art", {
      filePath: filePath,
      coverData: coverData,
    }),
  loadCoverArtFile: (filePath: string) =>
    bridge.invoke("load_cover_art_file", { filePath }),

  // Audio processing commands
  analyzeAudioFiles: (filePaths: string[]) =>
    bridge.invoke<FileListInfo>("analyze_audio_files", {
      filePaths: filePaths,
    }),
  // UI test functions
  testDisplayList: (fileListInfo: FileListInfo) =>
    displayFileList(fileListInfo),
  getCurrentFileList: () => currentFileList,
  clearFiles: () => clearAllFiles(),
  toggleSort: () => toggleFileSort(),
  // Test art thumbnail functionality
  testArtThumbnail: async () => {
    const statusPanel = getStatusPanel();
    if (statusPanel) {
      console.log("Testing art thumbnail update...");
      await (statusPanel as any).updateArtThumbnail();
      return "Art thumbnail test completed - check the progress panel";
    }
    return "StatusPanel not initialized";
  },

  // Output panel test functions
  getCurrentAudioSettings: () => getCurrentAudioSettings(),
  triggerFileListChange: () => onFileListChange(),
  triggerMetadataChange: () => onMetadataChange(),

  // Status panel test functions
  cancelProcessing: () => bridge.invoke("cancel_processing"),

  // Cover art test functions
  getCurrentCoverArt: () => getCurrentCoverArt(),
  setCoverArt: (coverArtBytes: number[] | null) => setCoverArt(coverArtBytes),
  clearCoverArt: () => clearCoverArt(),

  // File movement test functions
  testMoveFile: (index: number, direction: "up" | "down") => {
    if (direction === "up") {
      moveFileUp(index);
    } else if (direction === "down") {
      moveFileDown(index);
    }
  },
  testSortFiles: () => toggleFileSort(),

  // Tag preview test functions
  updateTagPreview: () => updateTagPreview(),
};

// Log when ready
console.log("Test commands available:");
console.log("  window.testCommands.ping()");
console.log("  window.testCommands.echo(input)");
console.log("  window.testCommands.validateFiles(paths)");
// Removed: getFFmpegVersion, mergeAudioFiles
console.log("  window.testCommands.readMetadata(filePath)");
console.log("  window.testCommands.writeMetadata(filePath, metadata)");
console.log("  window.testCommands.writeCoverArt(filePath, coverData)");
console.log("  window.testCommands.analyzeAudioFiles(filePaths)");
console.log("  window.testCommands.testDisplayList(fileListInfo)");
console.log("  window.testCommands.getCurrentFileList()");
console.log("  window.testCommands.clearFiles()");
console.log("  window.testCommands.getCurrentAudioSettings()");
console.log("  window.testCommands.triggerFileListChange()");
console.log("  window.testCommands.triggerMetadataChange()");
console.log("  window.testCommands.testArtThumbnail()");
console.log("  window.testCommands.loadCoverArtFile(filePath)");
console.log("  window.testCommands.getCurrentCoverArt()");
console.log("  window.testCommands.setCoverArt(coverArtBytes)");
console.log("  window.testCommands.clearCoverArt()");
console.log("  window.testCommands.testMoveFile(index, direction)");
console.log("  window.testCommands.testSortFiles()");
console.log("  window.testCommands.updateTagPreview()");

// Initialize UI components when DOM is ready
document.addEventListener("DOMContentLoaded", () => {
  initFileImport();
  initOutputPanel();
  initStatusPanel();
  initCoverArt();
  // Initialize Advanced Encoder panel (no-op if panel not present)
  initEncoderPanel();
  // Initialize tag preview grid
  initTagPreview();
  // Initialize Cmd+S metadata save handler
  initMetadataSaveHandler();
  console.log("File import system initialized");
  console.log("Output panel initialized");
  console.log("Status panel initialized");
  console.log("Cover art system initialized");
  console.log("Tag preview initialized");
  console.log("Metadata save handler initialized (Cmd+S / Ctrl+S)");
});

/**
 * Initializes the Cmd+S / Ctrl+S keyboard handler for metadata-only saving
 *
 * When triggered:
 * 1. Collects metadata from the form
 * 2. Computes TSOA on the backend
 * 3. Saves metadata to the loaded file(s) without audio processing
 */
function initMetadataSaveHandler(): void {
  document.addEventListener("keydown", async (event) => {
    // Check for Cmd+S (Mac) or Ctrl+S (Windows/Linux)
    if ((event.metaKey || event.ctrlKey) && event.key === "s") {
      event.preventDefault(); // Prevent browser save dialog

      // Check if we have files loaded
      if (!currentFileList || currentFileList.files.length === 0) {
        console.log("No files loaded - nothing to save");
        return;
      }

      // Check if processing is active
      const statusPanel = getStatusPanel();
      if (statusPanel?.isCurrentlyProcessing) {
        console.log("Processing in progress - cannot save metadata now");
        return;
      }

      // Get the file to save metadata to. Prioritize the selected file.
      let fileToSave = null;
      if (
        selectedFileIndex > -1 &&
        currentFileList.files[selectedFileIndex]?.isValid
      ) {
        fileToSave = currentFileList.files[selectedFileIndex];
      } else {
        fileToSave = currentFileList.files.find((f) => f.isValid);
      }

      if (!fileToSave) {
        console.log("No valid files to save metadata to");
        return;
      }

      // Collect metadata from the form
      const metadata = collectMetadataFromForm();

      try {
        console.log("Saving metadata to:", fileToSave.path);
        await bridge.invoke("save_metadata_to_file", {
          filePath: fileToSave.path,
          metadata: metadata,
        });
        console.log("Metadata saved successfully");

        // Update status text briefly to indicate success
        const statusText = document.getElementById("status-text");
        if (statusText) {
          const originalText = statusText.textContent;
          statusText.textContent = "Metadata saved!";
          // Fix race condition: only restore if still showing "Metadata saved!"
          setTimeout(() => {
            if (statusText.textContent === "Metadata saved!") {
              statusText.textContent = originalText;
            }
          }, 2000);
        }
      } catch (error) {
        console.error("Failed to save metadata:", error);
        // Show error to user
        const statusText = document.getElementById("status-text");
        if (statusText) {
          statusText.textContent = "Save failed - see console";
        }
      }
    }
  });
}

/**
 * Collects metadata from the form fields and returns an AudiobookMetadata object
 */
function collectMetadataFromForm(): AudiobookMetadata {
  const getElementValue = (id: string): string => {
    const element = document.getElementById(id) as
      | HTMLInputElement
      | HTMLTextAreaElement;
    return element?.value?.trim() || "";
  };

  const title = getElementValue("meta-title");
  const author = getElementValue("meta-author");
  const narrator = getElementValue("meta-narrator");
  const year = getElementValue("meta-year");
  const genre = getElementValue("meta-genre");
  const series = getElementValue("meta-series");
  const seriesPart = getElementValue("meta-series-part");
  const description = getElementValue("meta-description");

  const metadata: AudiobookMetadata = {};

  // Map form fields to Rust struct field names
  if (title) {
    metadata.title = title;
    metadata.album = title; // Album = Title for audiobooks
  }
  if (author) metadata.artist = author; // artist = Author
  if (narrator) metadata.composer = narrator; // composer = Narrator
  if (year) {
    const yearNum = parseInt(year);
    if (!isNaN(yearNum)) metadata.date = yearNum; // date = Year
  }
  if (genre) metadata.genre = genre;
  if (series) metadata.series = series;
  if (seriesPart) metadata.series_part = seriesPart;
  if (description) metadata.description = description;

  // Include cover art if present
  const coverBytes = getCurrentCoverArt();
  if (coverBytes && coverBytes.length > 0) {
    metadata.cover_art = coverBytes;
  }

  return metadata;
}
