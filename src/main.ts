import { bridge } from "./lib/bridge";
import type { AudiobookMetadata } from "./types/metadata";
import type { AudioFile } from "./types/audio";
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
import { getSelectedFileIndices } from "./ui/fileList/state";
import {
  initOutputPanel,
  getCurrentOutputConfig,
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
import { readMetadataForm, initMetadataFormEvents, resetDirtyState } from "./ui/metadataForm";
import { getSeriesPartValidationError } from "./ui/metadataValidation";
import { initTagPreview, updateTagPreview } from "./ui/tagPreview";
import { initJobControls } from "./ui/jobControls";
import { setMetadataForFile } from "./ui/metadataState";

// Initialize UI components when DOM is ready
document.addEventListener("DOMContentLoaded", () => {
  initFileImport();
  initOutputPanel();
  initStatusPanel();
  initCoverArt();
  initMetadataFormEvents();
  // Initialize Advanced Encoder panel (no-op if panel not present)
  initEncoderPanel();
  // Initialize tag preview grid
  initTagPreview();
  // Initialize Cmd+S metadata save handler
  initMetadataSaveHandler();
  // Initialize Job Controls (Job Type, Max Concurrent)
  initJobControls();
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
 * 1. Collects DIRTY metadata changes from the form
 * 2. Applies changes to ALL selected files (Read-Merge-Write pattern)
 * 3. Saves each file
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

      // Determine target files
      const selectedIndices = getSelectedFileIndices();
      let targetFiles: AudioFile[] = [];

      if (selectedIndices.size > 0) {
        targetFiles = Array.from(selectedIndices)
          .map((i) => currentFileList!.files[i])
          .filter((f) => f && f.isValid);
      } else {
        if (
          selectedFileIndex > -1 &&
          currentFileList.files[selectedFileIndex]?.isValid
        ) {
          targetFiles = [currentFileList.files[selectedFileIndex]];
        } else {
          const first = currentFileList.files.find((f) => f.isValid);
          if (first) targetFiles = [first];
        }
      }

      if (targetFiles.length === 0) {
        console.log("No valid files to save metadata to");
        return;
      }

      const isMultiSelect = selectedIndices.size > 1;
      const metadataPayload = isMultiSelect
        ? readMetadataForm({ mode: "multi", onlyDirty: true })
        : readMetadataForm({ mode: "single" });

      const hasChanges = Object.keys(metadataPayload).length > 0;
      if (!hasChanges && isMultiSelect) {
        console.log("No metadata changes detected (multi-select).");
        return;
      }

      const seriesPartError = getSeriesPartValidationError(
        typeof metadataPayload.series_part === "string"
          ? metadataPayload.series_part
          : undefined
      );
      if (seriesPartError) {
        const statusText = document.getElementById("status-text");
        if (statusText) {
          statusText.textContent = seriesPartError;
        }
        return;
      }

      try {
        let successCount = 0;

        if (isMultiSelect) {
          for (const file of targetFiles) {
            console.log(`Processing save for ${file.path}...`);

            const currentMeta = await bridge.invoke<AudiobookMetadata>(
              "read_audio_metadata",
              { filePath: file.path }
            );

            const merged = { ...currentMeta, ...metadataPayload };

            await bridge.invoke("save_metadata_to_file", {
              filePath: file.path,
              metadata: merged,
            });

            setMetadataForFile(file.path, merged);
            successCount++;
          }
        } else {
          const file = targetFiles[0];
          await bridge.invoke("save_metadata_to_file", {
            filePath: file.path,
            metadata: metadataPayload,
          });
          setMetadataForFile(file.path, metadataPayload);
          successCount = 1;
        }

        resetDirtyState();
        console.log(`Metadata saved successfully for ${successCount} files`);

        const statusText = document.getElementById("status-text");
        if (statusText) {
          const originalText = statusText.textContent;
          const msg =
            targetFiles.length > 1
              ? `Metadata saved (${successCount} files)!`
              : "Metadata saved!";
          statusText.textContent = msg;
          setTimeout(() => {
            if (statusText.textContent === msg) {
              statusText.textContent = originalText;
            }
          }, 2000);
        }
      } catch (error) {
        console.error("Failed to save metadata:", error);
        const statusText = document.getElementById("status-text");
        if (statusText) {
          statusText.textContent = "Save failed - see console";
        }
      }
    }
  });
}
