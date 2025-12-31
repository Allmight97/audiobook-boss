import { AudioFile, formatDuration, formatFileSize } from "../../types/audio";
import { currentFileList, isOrderLocked, getSelectedFileIndices } from "./state";
import { updateDropZoneState } from "../fileImport";

// Cached DOM elements (stable roots only)
let sortButton: HTMLElement | null = null;
let clearButton: HTMLElement | null = null;
let orderLockNotice: HTMLElement | null = null;

// Initialize cached DOM elements
export function initDOMCache(): void {
  sortButton = document.getElementById("sort-toggle-btn");
  clearButton = document.getElementById("clear-files-btn");
  orderLockNotice = document.getElementById("file-order-lock");
}

// Get container element - now uses persistent header pattern
function getContainer(): Element | null {
  return document.querySelector(".file-list-content");
}

export function createFileListItem(
  file: AudioFile,
  index: number
): HTMLElement {
  const item = document.createElement("div");
  item.className = `file-list-item ${file.isValid ? "valid" : "invalid"}`;
  item.dataset.index = index.toString();
  item.setAttribute("draggable", isOrderLocked() ? "false" : "true");
  item.setAttribute("role", "listitem");

  const fileName = file.path.split(/[\\\/]/).pop() || file.path;
  item.setAttribute("aria-label", fileName);
  const statusIcon = file.isValid ? "✓" : "✗";
  const statusClass = file.isValid ? "text-green-500" : "text-red-500";

  const isFirst = index === 0;
  const isLast = currentFileList
    ? index === currentFileList.files.length - 1
    : false;

  const locked = isOrderLocked();
  item.innerHTML = `
        <div class="file-item-content">
            <div class="file-status ${statusClass}">${statusIcon}</div>
            <div class="file-info">
                <div class="file-name">${fileName}</div>
                <div class="file-details">
                    ${file.isValid && file.duration && file.size
      ? `${formatDuration(file.duration)} • ${formatFileSize(
        file.size
      )} • ${file.format}`
      : `Error: ${file.error || "Invalid file"}`
    }
                </div>
            </div>
            <button class="move-up-btn" data-index="${index}" ${isFirst || locked ? "disabled" : ""
    }>▲</button>
            <button class="move-down-btn" data-index="${index}" ${isLast || locked ? "disabled" : ""
    }>▼</button>
            <button class="remove-file-btn" data-index="${index}" ${locked ? "disabled" : ""
    }>×</button>
        </div>
    `;

  return item;
}

export function updateFileListItem(
  item: HTMLElement,
  file: AudioFile,
  index: number
): void {
  item.className = `file-list-item ${file.isValid ? "valid" : "invalid"}`;
  item.dataset.index = index.toString();
  item.setAttribute("draggable", isOrderLocked() ? "false" : "true");
  item.setAttribute("role", "listitem");

  const fileName = file.path.split(/[\\\/]/).pop() || file.path;
  item.setAttribute("aria-label", fileName);
  const statusIcon = file.isValid ? "✓" : "✗";
  const statusClass = file.isValid ? "text-green-500" : "text-red-500";

  const isFirst = index === 0;
  const isLast = currentFileList
    ? index === currentFileList.files.length - 1
    : false;

  const locked = isOrderLocked();
  item.innerHTML = `
        <div class="file-item-content">
            <div class="file-status ${statusClass}">${statusIcon}</div>
            <div class="file-info">
                <div class="file-name">${fileName}</div>
                <div class="file-details">
                    ${file.isValid && file.duration && file.size
      ? `${formatDuration(file.duration)} • ${formatFileSize(
        file.size
      )} • ${file.format}`
      : `Error: ${file.error || "Invalid file"}`
    }
                </div>
            </div>
            <button class="move-up-btn" data-index="${index}" ${isFirst || locked ? "disabled" : ""
    }>▲</button>
            <button class="move-down-btn" data-index="${index}" ${isLast || locked ? "disabled" : ""
    }>▼</button>
            <button class="remove-file-btn" data-index="${index}" ${locked ? "disabled" : ""
    }>×</button>
        </div>
    `;
}

export function updateFileListDOM(): void {
  if (!currentFileList) return;

  const container = getContainer();
  if (!container) return;

  const hasFiles = currentFileList.files.length > 0;

  // Update drop zone state
  updateDropZoneState(hasFiles);

  // If no files, clear container
  if (!hasFiles) {
    container.innerHTML = "";

    // Hide sort button when no files
    if (sortButton) {
      sortButton.style.display = "none";
    }

    return;
  }

  // Remove excess items
  const existingItems = container.querySelectorAll(".file-list-item");
  for (let i = currentFileList.files.length; i < existingItems.length; i++) {
    existingItems[i].remove();
  }

  // Update or create items
  currentFileList.files.forEach((file, index) => {
    const existingItem = existingItems[index] as HTMLElement;
    if (existingItem) {
      updateFileListItem(existingItem, file, index);
    } else {
      const newItem = createFileListItem(file, index);
      container.appendChild(newItem);
    }
  });

  updateButtonVisibility();
  updateTotalStats();
  updateSelection();
}

export function updateButtonVisibility(): void {
  if (!currentFileList) return;
  const locked = isOrderLocked();

  // Update sort button visibility
  const sortBtn = sortButton as HTMLButtonElement | null;
  if (sortBtn) {
    sortBtn.style.display =
      currentFileList.files.length > 1 ? "block" : "none";
    sortBtn.disabled = locked;
  }
  const clearBtn = clearButton as HTMLButtonElement | null;
  if (clearBtn) {
    clearBtn.style.display =
      currentFileList.files.length > 0 ? "block" : "none";
    clearBtn.disabled = locked;
  }
}

export function updateTotalStats(): void {
  if (!currentFileList) return;

  const totalSizeEl = document.getElementById("prop-combinedsize");
  if (totalSizeEl)
    totalSizeEl.textContent = formatFileSize(currentFileList.totalSize);
}


export function updateSelection(): void {
  const selectedIndices = getSelectedFileIndices();
  const items = document.querySelectorAll(".file-list-item");
  items.forEach((item, index) => {
    item.classList.toggle("selected", selectedIndices.has(index));
  });
}

export function updateSortButtonText(ascending: boolean): void {
  if (sortButton) {
    sortButton.textContent = ascending ? "Sort: A-Z" : "Sort: Z-A";
  }
}

export function setOrderLockNotice(locked: boolean): void {
  if (!orderLockNotice) return;
  orderLockNotice.style.display = locked ? "inline" : "none";
}

export function clearContainer(): void {
  const container = getContainer();
  if (container) {
    container.innerHTML = "";
  }
  updateDropZoneState(false);
}

export function showEmptyState(): void {
  clearContainer();

  // Hide buttons when no files
  if (sortButton) {
    sortButton.style.display = "none";
  }
  if (clearButton) {
    clearButton.style.display = "none";
  }
}
