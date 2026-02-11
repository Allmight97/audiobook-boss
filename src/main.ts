import { bridge } from "./lib/bridge";
import type { AudioFile } from "./types/audio";
import { initFileImport } from "./ui/fileImport";
import { getCurrentFileList, getSelectedFileIndex } from "./ui/fileList";
import { getSelectedFileIndices } from "./ui/fileList/state";
import { initOutputPanel } from "./ui/outputPanel";
import { initStatusPanel, getStatusPanel } from "./ui/statusPanel/index";
import { initEncoderPanel } from "./ui/encoderPanel";
import { initCoverArt } from "./ui/coverArt";
import { readMetadataForm, initMetadataFormEvents, resetDirtyState } from "./ui/metadataForm";
import {
  getSeriesPartValidationError,
  getSubseriesPartValidationError,
} from "./ui/metadataValidation";
import { initTagPreview } from "./ui/tagPreview";
import { initJobControls } from "./ui/jobControls";
import { initMetadataLookup } from "./ui/metadataLookup";
import { setMetadataForFile, getMetadataForFile } from "./ui/metadataState";
import { isMetadataSaveInProgress, setMetadataSaveInProgress } from "./ui/metadataSaveState";

// Initialize UI components when DOM is ready
document.addEventListener("DOMContentLoaded", () => {
  initFileImport();
  // Initialize Advanced Encoder panel before output handlers so shared controls are present.
  initEncoderPanel();
  initOutputPanel();
  initStatusPanel();
  initCoverArt();
  initMetadataFormEvents();
  // Initialize tag preview grid
  initTagPreview();
  initMetadataLookup();
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

async function saveMetadataFromUI(): Promise<void> {
  const fileList = getCurrentFileList();
  // Check if we have files loaded
  if (!fileList || fileList.files.length === 0) {
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
      .map((i) => fileList.files[i])
      .filter((f) => f && f.isValid);
  } else {
    const selectedFileIndex = getSelectedFileIndex();
    if (
      selectedFileIndex > -1 &&
      fileList.files[selectedFileIndex]?.isValid
    ) {
      targetFiles = [fileList.files[selectedFileIndex]];
    } else {
      const first = fileList.files.find((f) => f.isValid);
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
  const subseriesPartError = getSubseriesPartValidationError(
    typeof metadataPayload.subseries_part === "string"
      ? metadataPayload.subseries_part
      : undefined
  );
  const validationError = seriesPartError ?? subseriesPartError;
  if (validationError) {
    const statusText = document.getElementById("status-text");
    if (statusText) {
      statusText.textContent = validationError;
    }
    return;
  }

  const statusText = document.getElementById("status-text");
  const originalText = statusText?.textContent ?? "";
  if (statusText) {
    statusText.textContent = "Saving metadata...";
  }

  if (isMetadataSaveInProgress()) {
    if (statusText) {
      statusText.textContent = "Save already in progress...";
    }
    return;
  }

  try {
    setMetadataSaveInProgress(true);
    let successCount = 0;

    if (isMultiSelect) {
      for (const [index, file] of targetFiles.entries()) {
        console.log(`Processing save for ${file.path}...`);

        if (statusText) {
          statusText.textContent = `Saving ${index + 1}/${targetFiles.length}...`;
        }

        await bridge.saveMetadataToFile(file.path, metadataPayload);

        const existing = getMetadataForFile(file.path) ?? {};
        setMetadataForFile(file.path, { ...existing, ...metadataPayload });
        successCount++;
      }
    } else {
      const file = targetFiles[0];
      await bridge.saveMetadataToFile(file.path, metadataPayload);
      setMetadataForFile(file.path, metadataPayload);
      successCount = 1;
    }

    resetDirtyState();
    console.log(`Metadata saved successfully for ${successCount} files`);

    if (statusText) {
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
    if (statusText) {
      statusText.textContent = "Save failed - see console";
    }
  } finally {
    setMetadataSaveInProgress(false);
  }
}

function initMetadataSaveHandler(): void {
  const saveButton = document.getElementById(
    "metadata-save-btn"
  ) as HTMLButtonElement | null;
  if (saveButton) {
    saveButton.addEventListener("click", () => {
      void saveMetadataFromUI();
    });
  }

  document.addEventListener("keydown", (event) => {
    // Check for Cmd+S (Mac) or Ctrl+S (Windows/Linux)
    if ((event.metaKey || event.ctrlKey) && event.key === "s") {
      event.preventDefault(); // Prevent browser save dialog
      void saveMetadataFromUI();
    }
  });
}
