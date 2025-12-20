import { AudioFile, FileListInfo, formatFileSize } from "../../types/audio";
import { bridge } from "../../lib/bridge";
import { onFileListChange, onMetadataChange } from "../outputPanel";
import { setCoverArt, getHasCustomCoverArt, clearCoverArt } from "../coverArt";
import type { AudiobookMetadata } from "../../types/metadata";
import { readMetadataForm } from "../metadataForm";
import {
  clearMetadataState,
  getMetadataForFile,
  removeMetadataForFile,
  setMetadataForFile,
} from "../metadataState";
import {
  currentFileList,
  selectedFileIndex,
  setCurrentFileList,
  setSelectedIndex,
  getSortAscending,
  setSortAscending,
} from "./state";
import {
  updateFileListDOM,
  updateTotalStats,
  updateSelection,
  updateSortButtonText,
  updateButtonVisibility,
  showEmptyState,
} from "./dom";
import { initFileListEvents, setupDragStartHandlers } from "./events";

export function displayFileList(fileListInfo: FileListInfo): void {
  clearMetadataState();
  setCurrentFileList(fileListInfo);

  // Update DOM (includes drop zone state update)
  updateFileListDOM();

  // Initialize events (includes drag handlers)
  initFileListEvents();

  // Update stats and buttons
  updateTotalStats();
  updateButtonVisibility();
  updateSortButtonText(getSortAscending());

  // Notify other components
  onFileListChange();

  // Auto-load cover art from the first valid file unless the user has
  // already provided custom cover art
  void autoUpdateCoverArtFromFirstValidFile();
}

export function selectFile(index: number): void {
  if (!currentFileList || index < 0 || index >= currentFileList.files.length)
    return;

  const previousFile =
    selectedFileIndex >= 0 ? currentFileList.files[selectedFileIndex] : null;
  if (previousFile?.isValid) {
    setMetadataForFile(previousFile.path, readMetadataForm());
  }

  setSelectedIndex(index);
  updateSelection();
  const nextFile = currentFileList.files[index];
  const storedMetadata = getMetadataForFile(nextFile.path);
  if (storedMetadata) {
    updateFileProperties(nextFile, { skipMetadataLoad: true });
    populateMetadataForm(storedMetadata);
    return;
  }
  updateFileProperties(nextFile);
}

export function removeFile(index: number): void {
  if (!currentFileList || index < 0 || index >= currentFileList.files.length)
    return;

  const removedFile = currentFileList.files[index];
  removeMetadataForFile(removedFile.path);

  currentFileList.files.splice(index, 1);
  currentFileList.validCount = currentFileList.files.filter(
    (f) => f.isValid
  ).length;
  currentFileList.invalidCount =
    currentFileList.files.length - currentFileList.validCount;

  recalculateTotals();
  updateFileListDOM();

  if (selectedFileIndex === index) {
    setSelectedIndex(-1);
    clearFileProperties();
  } else if (selectedFileIndex > index) {
    setSelectedIndex(selectedFileIndex - 1);
  }

  onFileListChange();
}

export function recalculateTotals(): void {
  if (!currentFileList) return;

  const validFiles = currentFileList.files.filter(
    (f) => f.isValid && f.duration && f.size
  );
  currentFileList.totalDuration = validFiles.reduce(
    (sum, f) => sum + (f.duration || 0),
    0
  );
  currentFileList.totalSize = validFiles.reduce(
    (sum, f) => sum + (f.size || 0),
    0
  );
}

export function updateFileProperties(
  file: AudioFile,
  options?: { skipMetadataLoad?: boolean }
): void {
  const bitrateEl = document.getElementById("prop-bitrate");
  const sampleRateEl = document.getElementById("prop-samplerate");
  const channelsEl = document.getElementById("prop-channels");
  const fileSizeEl = document.getElementById("prop-filesize");

  if (file.isValid) {
    // Display technical audio properties
    if (bitrateEl)
      bitrateEl.textContent = file.bitrate ? `${file.bitrate} kb/s` : "N/A";
    if (sampleRateEl)
      sampleRateEl.textContent = file.sampleRate
        ? `${file.sampleRate} Hz`
        : "N/A";
    if (channelsEl)
      channelsEl.textContent = file.channels ? `${file.channels} ch` : "N/A";
    if (fileSizeEl)
      fileSizeEl.textContent = file.size ? formatFileSize(file.size) : "N/A";

    if (!options?.skipMetadataLoad) {
      // Still load metadata for the metadata form
      loadFileMetadata(file.path);
    }
  } else {
    // File is invalid, show dashes
    if (bitrateEl) bitrateEl.textContent = "---";
    if (sampleRateEl) sampleRateEl.textContent = "---";
    if (channelsEl) channelsEl.textContent = "---";
    if (fileSizeEl) fileSizeEl.textContent = "---";
  }

  // Update context header
  updatePropertiesContext(file, selectedFileIndex);
}

function updatePropertiesContext(file: AudioFile, index: number): void {
  const contextEl = document.getElementById("prop-selected-context");
  if (!contextEl) return;

  // Clear existing content using replaceChildren() for better DX (structural change only)
  contextEl.replaceChildren();

  if (!currentFileList || index < 0 || index >= currentFileList.files.length) {
    // Empty state
    const emptySpan = document.createElement("span");
    emptySpan.className = "context-empty";
    emptySpan.textContent = "No file selected";
    contextEl.appendChild(emptySpan);
    return;
  }

  // File selected state - build structured display
  const fileName = file.path.split(/[\\\/]/).pop() || file.path;
  const totalFiles = currentFileList.files.length;

  // Filename span with truncation
  const filenameSpan = document.createElement("span");
  filenameSpan.className = "context-filename";
  filenameSpan.title = fileName; // Full name on hover
  filenameSpan.textContent = fileName;

  // Position badge
  const posSpan = document.createElement("span");
  posSpan.className = "context-position";
  posSpan.textContent = `${index + 1} of ${totalFiles}`;

  contextEl.appendChild(filenameSpan);
  contextEl.appendChild(posSpan);
}

async function loadFileMetadata(filePath: string): Promise<void> {
  try {
    const metadata = await bridge.invoke<AudiobookMetadata>(
      "read_audio_metadata",
      { filePath: filePath }
    );
    setMetadataForFile(filePath, metadata);
    populateMetadataForm(metadata);
  } catch (error) {
    console.warn("Failed to load metadata:", error);
  }
}

/**
 * Populates the metadata form with values from the backend
 *
 * Field mapping from Rust AudiobookMetadata:
 * - artist → Author
 * - composer → Narrator
 * - date → Year
 * - series → Series
 * - series_part → Book #
 */
function populateMetadataForm(metadata: Partial<AudiobookMetadata>): void {
  const titleEl = document.getElementById("meta-title") as HTMLInputElement;
  const authorEl = document.getElementById("meta-author") as HTMLInputElement;
  const narratorEl = document.getElementById(
    "meta-narrator"
  ) as HTMLInputElement;
  const yearEl = document.getElementById("meta-year") as HTMLInputElement;
  const genreEl = document.getElementById("meta-genre") as HTMLInputElement;
  const seriesEl = document.getElementById("meta-series") as HTMLInputElement;
  const seriesPartEl = document.getElementById(
    "meta-series-part"
  ) as HTMLInputElement;
  const descriptionEl = document.getElementById(
    "meta-description"
  ) as HTMLTextAreaElement;

  // Clear existing values first to avoid stale data
  if (titleEl) titleEl.value = "";
  if (authorEl) authorEl.value = "";
  if (narratorEl) narratorEl.value = "";
  if (yearEl) yearEl.value = "";
  if (genreEl) genreEl.value = "";
  if (seriesEl) seriesEl.value = "";
  if (seriesPartEl) seriesPartEl.value = "";
  if (descriptionEl) descriptionEl.value = "";

  // Populate with new values using correct Rust field names
  if (titleEl && metadata.title) titleEl.value = metadata.title;
  if (authorEl && metadata.artist) authorEl.value = metadata.artist; // artist = Author
  if (narratorEl && metadata.composer) narratorEl.value = metadata.composer; // composer = Narrator
  if (yearEl && metadata.date) yearEl.value = metadata.date.toString(); // date = Year
  if (genreEl && metadata.genre) genreEl.value = metadata.genre;
  if (seriesEl && metadata.series) seriesEl.value = metadata.series;
  if (seriesPartEl && metadata.series_part)
    seriesPartEl.value = metadata.series_part;
  if (descriptionEl && metadata.description)
    descriptionEl.value = metadata.description;

  // Handle cover art display - preserve user-loaded custom art
  if (!getHasCustomCoverArt()) {
    setCoverArt(metadata.cover_art || null);
  }

  // Update the output path preview now that metadata has changed
  onMetadataChange();
}

async function autoUpdateCoverArtFromFirstValidFile(): Promise<void> {
  try {
    if (getHasCustomCoverArt()) return;
    if (!currentFileList || !currentFileList.files.length) {
      setCoverArt(null);
      return;
    }
    const firstValid = currentFileList.files.find((f) => f.isValid);
    if (!firstValid) {
      setCoverArt(null);
      return;
    }
    const metadata = await bridge.invoke<AudiobookMetadata>(
      "read_audio_metadata",
      { filePath: firstValid.path }
    );
    setCoverArt(metadata.cover_art || null);
  } catch (error) {
    // Non-fatal: just reset to placeholder if metadata cannot be read
    setCoverArt(null);
    console.warn("Failed to auto-load cover art:", error);
  }
}

// Move file up in the list
export function moveFileUp(index: number): void {
  if (!currentFileList || index <= 0 || index >= currentFileList.files.length)
    return;

  // Swap with previous file
  const temp = currentFileList.files[index];
  currentFileList.files[index] = currentFileList.files[index - 1];
  currentFileList.files[index - 1] = temp;

  // Update selected index if needed
  if (selectedFileIndex === index) {
    setSelectedIndex(index - 1);
  } else if (selectedFileIndex === index - 1) {
    setSelectedIndex(index);
  }

  updateFileListDOM();
  setupDragStartHandlers();
  onFileListChange();
}

// Move file down in the list
export function moveFileDown(index: number): void {
  if (
    !currentFileList ||
    index < 0 ||
    index >= currentFileList.files.length - 1
  )
    return;

  // Swap with next file
  const temp = currentFileList.files[index];
  currentFileList.files[index] = currentFileList.files[index + 1];
  currentFileList.files[index + 1] = temp;

  // Update selected index if needed
  if (selectedFileIndex === index) {
    setSelectedIndex(index + 1);
  } else if (selectedFileIndex === index + 1) {
    setSelectedIndex(index);
  }

  updateFileListDOM();
  setupDragStartHandlers();
  onFileListChange();
}

export function clearFileProperties(): void {
  const bitrateEl = document.getElementById("prop-bitrate");
  const sampleRateEl = document.getElementById("prop-samplerate");
  const channelsEl = document.getElementById("prop-channels");
  const fileSizeEl = document.getElementById("prop-filesize");

  if (bitrateEl) bitrateEl.textContent = "---";
  if (sampleRateEl) sampleRateEl.textContent = "---";
  if (channelsEl) channelsEl.textContent = "---";
  if (fileSizeEl) fileSizeEl.textContent = "---";

  const contextEl = document.getElementById("prop-selected-context");
  if (contextEl) {
    contextEl.replaceChildren();
    const emptySpan = document.createElement("span");
    emptySpan.className = "context-empty";
    emptySpan.textContent = "No file selected";
    contextEl.appendChild(emptySpan);
  }

  // Clear metadata form
  const titleEl = document.getElementById("meta-title") as HTMLInputElement;
  const authorEl = document.getElementById("meta-author") as HTMLInputElement;
  const narratorEl = document.getElementById(
    "meta-narrator"
  ) as HTMLInputElement;
  const yearEl = document.getElementById("meta-year") as HTMLInputElement;
  const genreEl = document.getElementById("meta-genre") as HTMLInputElement;
  const seriesEl = document.getElementById("meta-series") as HTMLInputElement;
  const seriesPartEl = document.getElementById(
    "meta-series-part"
  ) as HTMLInputElement;
  const descriptionEl = document.getElementById(
    "meta-description"
  ) as HTMLTextAreaElement;

  if (titleEl) titleEl.value = "";
  if (authorEl) authorEl.value = "";
  if (narratorEl) narratorEl.value = "";
  if (yearEl) yearEl.value = "";
  if (genreEl) genreEl.value = "";
  if (seriesEl) seriesEl.value = "";
  if (seriesPartEl) seriesPartEl.value = "";
  if (descriptionEl) descriptionEl.value = "";

  // Clear cover art display and reset custom-art flag
  clearCoverArt();
}

export function toggleFileSort(): void {
  if (!currentFileList || currentFileList.files.length <= 1) return;

  setSortAscending(!getSortAscending());

  // Sort files by name
  currentFileList.files.sort((a, b) => {
    const nameA = a.path.split(/[\\\/]/).pop() || a.path;
    const nameB = b.path.split(/[\\\/]/).pop() || b.path;

    if (getSortAscending()) {
      return nameA.localeCompare(nameB);
    } else {
      return nameB.localeCompare(nameA);
    }
  });

  // Reset selected index as files have been reordered
  setSelectedIndex(-1);
  clearFileProperties();

  // Update the sort button text
  updateSortButtonText(getSortAscending());

  // Ensure button visibility is updated after reordering
  updateButtonVisibility();

  updateFileListDOM();
  setupDragStartHandlers();
  onFileListChange();
}

export function clearAllFiles(): void {
  if (!currentFileList) return;

  clearMetadataState();
  currentFileList.files = [];
  currentFileList.validCount = 0;
  currentFileList.invalidCount = 0;
  currentFileList.totalDuration = 0;
  currentFileList.totalSize = 0;

  showEmptyState();

  setSelectedIndex(-1);
  clearFileProperties();
  updateTotalStats();
  updateButtonVisibility();
  onFileListChange();
}
