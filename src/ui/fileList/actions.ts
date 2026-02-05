import type { AudioFile, FileListInfo } from "../../types/audio";
import {
  onFileListChange,
  onMetadataChange,
} from "../outputPanel";
import {
  clearMetadataState,
  getMetadataForFile,
  removeMetadataForFile,
  setMetadataForFile,
} from "../metadataState";
import {
  getSeriesPartValidationError,
  getSubseriesPartValidationError,
} from "../metadataValidation";
import {
  readMetadataForm,
  resetDirtyState,
} from "../metadataForm";
import {
  currentFileList,
  selectedFileIndex,
  setCurrentFileList,
  setSelectedIndex,
  getSortAscending,
  setSortAscending,
  isOrderLocked,
  setOrderLocked,
} from "./state";
import {
  updateFileListDOM,
  updateTotalStats,
  updateSelection,
  updateSortButtonText,
  updateButtonVisibility,
  showEmptyState,
  setOrderLockNotice,
} from "./dom";
import { initFileListEvents, setupDragStartHandlers } from "./events";
import {
  clearSelection,
  handleSelection,
  reindexSelectionAfterMove,
  reindexSelectionAfterRemoval,
  selectAllFiles,
  swapSelectionIndices,
} from "./selection";
import {
  autoUpdateCoverArtFromFirstValidFile,
  clearSelectionPanels,
  ensureMetadataForFiles,
  getSelectedFiles,
  showMultiSelection,
  showSingleSelection,
} from "./metadataPanel";

export function displayFileList(fileListInfo: FileListInfo): void {
  clearMetadataState();
  setCurrentFileList(fileListInfo);

  updateFileListDOM();
  initFileListEvents();

  updateTotalStats();
  updateButtonVisibility();
  updateSortButtonText(getSortAscending());

  onFileListChange();

  void autoUpdateCoverArtFromFirstValidFile();
}

function persistSingleSelectionMetadata(file: AudioFile | null): void {
  if (!file?.isValid) return;

  const metadata = readMetadataForm({ mode: "single" });
  setMetadataForFile(file.path, metadata);
}

export async function selectFile(
  index: number,
  modifiers?: { multi: boolean; range: boolean },
  options?: { skipPersistPrevious?: boolean }
): Promise<void> {
  if (!currentFileList || index < 0 || index >= currentFileList.files.length) {
    return;
  }

  const previousSelectionCount = getSelectedFiles().length;
  const previousIndex = selectedFileIndex;
  const previousFile =
    previousSelectionCount === 1 && currentFileList
      ? currentFileList.files[previousIndex] ?? null
      : null;

  const selectionResult = handleSelection(index, modifiers || { multi: false, range: false });
  if (!selectionResult.changed) return;

  if (
    previousSelectionCount === 1 &&
    previousIndex >= 0 &&
    !options?.skipPersistPrevious
  ) {
    persistSingleSelectionMetadata(previousFile);
  }

  updateSelection();

  const selectedFiles = getSelectedFiles();
  const count = selectedFiles.length;

  if (previousSelectionCount > 1 && count <= 1) {
    await stageMetadataToSelection({ showStatus: false });
  }

  if (count === 0) {
    setSelectedIndex(-1);
    clearSelectionPanels();
    return;
  }

  if (count === 1) {
    void showSingleSelection(selectedFiles[0]);
    return;
  }

  void showMultiSelection(selectedFiles);
}

export function selectAll(): void {
  if (!currentFileList) return;
  const previousFile =
    getSelectedFiles().length === 1 && selectedFileIndex >= 0
      ? currentFileList.files[selectedFileIndex] ?? null
      : null;
  persistSingleSelectionMetadata(previousFile);

  const changed = selectAllFiles();
  if (!changed) return;

  updateSelection();
  const selectedFiles = getSelectedFiles();
  if (selectedFiles.length > 1) {
    void showMultiSelection(selectedFiles);
  } else if (selectedFiles.length === 1) {
    void showSingleSelection(selectedFiles[0]);
  }
}

export async function clearSelectionAction(): Promise<void> {
  const fileList = currentFileList;
  const previousSelectionCount = getSelectedFiles().length;
  const previousFile =
    fileList && previousSelectionCount === 1 && selectedFileIndex >= 0
      ? fileList.files[selectedFileIndex] ?? null
      : null;
  persistSingleSelectionMetadata(previousFile);

  if (previousSelectionCount > 1) {
    await stageMetadataToSelection({ showStatus: false });
  }

  const changed = clearSelection();
  if (!changed) return;

  updateSelection();
  clearSelectionPanels();
}


export async function stageMetadataToSelection(options?: {
  showStatus?: boolean;
}): Promise<boolean> {
  if (!currentFileList) return false;

  const selectedFiles = getSelectedFiles().filter((file) => file.isValid);
  if (selectedFiles.length === 0) return false;

  const changes = readMetadataForm({ mode: "multi", onlyDirty: true });
  if (Object.keys(changes).length === 0) {
    if (options?.showStatus) {
      const statusText = document.getElementById("status-text");
      if (statusText) {
        statusText.textContent = "No metadata changes to apply";
      }
    }
    return false;
  }

  const seriesPartError = getSeriesPartValidationError(
    typeof changes.series_part === "string" ? changes.series_part : undefined
  );
  const subseriesPartError = getSubseriesPartValidationError(
    typeof changes.subseries_part === "string" ? changes.subseries_part : undefined
  );
  const validationError = seriesPartError ?? subseriesPartError;
  if (validationError) {
    if (options?.showStatus) {
      const statusText = document.getElementById("status-text");
      if (statusText) {
        statusText.textContent = validationError;
      }
    }
    return false;
  }

  await ensureMetadataForFiles(selectedFiles);

  selectedFiles.forEach((file) => {
    const existing = getMetadataForFile(file.path) ?? {};
    const merged = { ...existing, ...changes };
    setMetadataForFile(file.path, merged);
  });

  resetDirtyState();
  onMetadataChange();

  if (options?.showStatus) {
    const statusText = document.getElementById("status-text");
    if (statusText) {
      const originalText = statusText.textContent;
      const msg = `Applied to ${selectedFiles.length} files`;
      statusText.textContent = msg;
      setTimeout(() => {
        if (statusText.textContent === msg) {
          statusText.textContent = originalText;
        }
      }, 2000);
    }
  }

  return true;
}

async function applyMetadataToSelection(): Promise<void> {
  await stageMetadataToSelection({ showStatus: true });
}

export function initMetadataApplyHandler(): void {
  const applyButton = document.getElementById(
    "metadata-apply-btn"
  ) as HTMLButtonElement | null;
  if (!applyButton) return;

  applyButton.addEventListener("click", () => {
    void applyMetadataToSelection();
  });
}

export function removeFile(index: number): void {
  if (isOrderLocked()) return;
  if (!currentFileList || index < 0 || index >= currentFileList.files.length) {
    return;
  }

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

  reindexSelectionAfterRemoval(index);
  updateSelection();

  const remainingSelection = getSelectedFiles();
  if (remainingSelection.length === 0) {
    clearSelectionPanels();
  } else if (remainingSelection.length === 1) {
    void showSingleSelection(remainingSelection[0]);
  } else {
    void showMultiSelection(remainingSelection);
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

export function moveFileUp(index: number): void {
  if (isOrderLocked()) return;
  if (!currentFileList || index <= 0 || index >= currentFileList.files.length) {
    return;
  }

  const temp = currentFileList.files[index];
  currentFileList.files[index] = currentFileList.files[index - 1];
  currentFileList.files[index - 1] = temp;

  swapSelectionIndices(index, index - 1);

  updateFileListDOM();
  setupDragStartHandlers();
  onFileListChange();

  const selectedFiles = getSelectedFiles();
  if (selectedFiles.length === 1) {
    void showSingleSelection(selectedFiles[0]);
  }
}

export function moveFileDown(index: number): void {
  if (isOrderLocked()) return;
  if (
    !currentFileList ||
    index < 0 ||
    index >= currentFileList.files.length - 1
  ) {
    return;
  }

  const temp = currentFileList.files[index];
  currentFileList.files[index] = currentFileList.files[index + 1];
  currentFileList.files[index + 1] = temp;

  swapSelectionIndices(index, index + 1);

  updateFileListDOM();
  setupDragStartHandlers();
  onFileListChange();

  const selectedFiles = getSelectedFiles();
  if (selectedFiles.length === 1) {
    void showSingleSelection(selectedFiles[0]);
  }
}

export function toggleFileSort(): void {
  if (isOrderLocked()) return;
  if (!currentFileList || currentFileList.files.length <= 1) return;

  setSortAscending(!getSortAscending());

  currentFileList.files.sort((a, b) => {
    const nameA = a.path.split(/[\\/]/).pop() || a.path;
    const nameB = b.path.split(/[\\/]/).pop() || b.path;

    if (getSortAscending()) {
      return nameA.localeCompare(nameB);
    }
    return nameB.localeCompare(nameA);
  });

  clearSelection();
  setSelectedIndex(-1);
  clearSelectionPanels();

  updateSortButtonText(getSortAscending());
  updateButtonVisibility();

  updateFileListDOM();
  setupDragStartHandlers();
  onFileListChange();
}

export function clearAllFiles(): void {
  if (isOrderLocked()) return;
  if (!currentFileList) return;

  clearMetadataState();
  currentFileList.files = [];
  currentFileList.validCount = 0;
  currentFileList.invalidCount = 0;
  currentFileList.totalDuration = 0;
  currentFileList.totalSize = 0;

  showEmptyState();

  clearSelection();
  setSelectedIndex(-1);
  clearSelectionPanels();
  updateTotalStats();
  updateButtonVisibility();
  onFileListChange();
}

export function setFileOrderLocked(locked: boolean): void {
  setOrderLocked(locked);
  setOrderLockNotice(locked);
  updateButtonVisibility();
  updateFileListDOM();
}

export function reorderFiles(fromIndex: number, toIndex: number): void {
  if (isOrderLocked()) return;
  if (!currentFileList) return;

  const files = currentFileList.files;
  const [moved] = files.splice(fromIndex, 1);
  files.splice(toIndex, 0, moved);

  reindexSelectionAfterMove(fromIndex, toIndex);

  updateFileListDOM();
  onFileListChange();
  setupDragStartHandlers();

  const selectedFiles = getSelectedFiles();
  if (selectedFiles.length === 1) {
    void showSingleSelection(selectedFiles[0]);
  } else if (selectedFiles.length > 1) {
    void showMultiSelection(selectedFiles);
  } else {
    clearSelectionPanels();
  }
}
