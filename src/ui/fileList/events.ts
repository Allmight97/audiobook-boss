import {
  currentFileList,
  selectedFileIndex,
  setSelectedIndex,
  isOrderLocked,
} from "./state";
import { selectFile, removeFile, moveFileUp, moveFileDown } from "./actions";
import { updateFileListDOM } from "./dom";
import { onFileListChange } from "../outputPanel";

let draggedIndex: number | null = null;

export function initFileListEvents(): void {
  const container = document.querySelector<HTMLElement>(".file-list-content");
  if (!container) return;

  // Remove any existing event listeners to prevent duplicates
  container.removeEventListener("click", handleFileListClick);
  container.removeEventListener("dragover", handleDragOver);
  container.removeEventListener("drop", handleDrop);
  container.removeEventListener("dragend", handleDragEnd);

  // Add event delegation handlers
  container.addEventListener("click", handleFileListClick);
  container.addEventListener("dragover", handleDragOver);
  container.addEventListener("drop", handleDrop);
  container.addEventListener("dragend", handleDragEnd);

  // Add dragstart handlers to each file item
  setupDragStartHandlers();
}

export function setupDragStartHandlers(): void {
  const container = document.querySelector<HTMLElement>(".file-list-content");
  if (!container) return;

  const items = container.querySelectorAll<HTMLElement>(".file-list-item");
  items.forEach((item, index) => {
    // Remove existing dragstart listeners by cloning (clean slate)
    const existingHandler = (item as any).__dragStartHandler;
    if (existingHandler) {
      item.removeEventListener("dragstart", existingHandler);
    }

    // Create new handler and store reference
    const handler = (e: DragEvent) => handleDragStart(e, index);
    (item as any).__dragStartHandler = handler;
    item.addEventListener("dragstart", handler);
  });
}

function handleFileListClick(e: Event): void {
  const target = e.target as HTMLElement;

  // Handle remove button clicks
  if (target.classList.contains("remove-file-btn")) {
    e.stopPropagation();
    e.preventDefault();
    if (isOrderLocked()) return;
    const index = parseInt(target.dataset.index || "-1");
    if (index >= 0) {
      console.log("Remove button clicked for index:", index);
      removeFile(index);
    }
    return;
  }

  // Handle move up button clicks
  if (target.classList.contains("move-up-btn")) {
    e.stopPropagation();
    e.preventDefault();
    if (isOrderLocked()) return;
    const index = parseInt(target.dataset.index || "-1");
    if (index > 0) {
      moveFileUp(index);
    }
    return;
  }

  // Handle move down button clicks
  if (target.classList.contains("move-down-btn")) {
    e.stopPropagation();
    e.preventDefault();
    if (isOrderLocked()) return;
    const index = parseInt(target.dataset.index || "-1");
    if (
      index >= 0 &&
      currentFileList &&
      index < currentFileList.files.length - 1
    ) {
      moveFileDown(index);
    }
    return;
  }

  // Handle file item selection
  const fileItem = target.closest(".file-list-item") as HTMLElement;
  if (fileItem) {
    const index = parseInt(fileItem.dataset.index || "-1");
    if (index >= 0) selectFile(index);
  }
}

function handleDragStart(e: DragEvent, index: number): void {
  if (isOrderLocked()) return;
  if (!e.dataTransfer) return;

  draggedIndex = index;
  e.dataTransfer.effectAllowed = "move";
  e.dataTransfer.setData("text/plain", index.toString());

  const item = e.currentTarget as HTMLElement;
  item.classList.add("dragging");
}

function handleDragOver(e: DragEvent): void {
  if (isOrderLocked()) return;
  e.preventDefault();
  if (!e.dataTransfer) return;

  e.dataTransfer.dropEffect = "move";

  const container = e.currentTarget as HTMLElement;
  const items = Array.from(container.querySelectorAll(".file-list-item"));

  // Remove all drag-over classes
  items.forEach((item) => item.classList.remove("drag-over"));

  // Find item under cursor
  const target = e.target as HTMLElement;
  const fileItem = target.closest(".file-list-item") as HTMLElement;
  if (fileItem && !fileItem.classList.contains("dragging")) {
    fileItem.classList.add("drag-over");
  }
}

function handleDrop(e: DragEvent): void {
  if (isOrderLocked()) return;
  e.preventDefault();
  e.stopPropagation();

  if (draggedIndex === null) return;

  const container = e.currentTarget as HTMLElement;
  const items = Array.from(container.querySelectorAll(".file-list-item"));

  // Remove drag-over classes
  items.forEach((item) => item.classList.remove("drag-over"));

  // Find drop target
  const target = e.target as HTMLElement;
  const dropTarget = target.closest(".file-list-item") as HTMLElement;
  if (!dropTarget) return;

  const dropIndex = parseInt(dropTarget.dataset.index || "-1");
  if (dropIndex < 0 || dropIndex === draggedIndex) return;

  // Reorder files
  reorderFiles(draggedIndex, dropIndex);

  draggedIndex = null;
}

function handleDragEnd(e: DragEvent): void {
  // Find the dragged item (it has the "dragging" class)
  // e.currentTarget is the container, so we need to find the actual dragged item
  const container = e.currentTarget as HTMLElement;
  const draggedItem = container.querySelector(".file-list-item.dragging");
  if (draggedItem) {
    draggedItem.classList.remove("dragging");
  }

  // Remove all drag-over classes
  container.querySelectorAll(".file-list-item").forEach((item) => {
    item.classList.remove("drag-over");
  });

  draggedIndex = null;
}

function reorderFiles(fromIndex: number, toIndex: number): void {
  if (isOrderLocked()) return;
  if (!currentFileList) return;

  const files = currentFileList.files;
  const [moved] = files.splice(fromIndex, 1);
  files.splice(toIndex, 0, moved);

  // Update selected index if needed
  if (selectedFileIndex === fromIndex) {
    setSelectedIndex(toIndex);
  } else if (selectedFileIndex === toIndex && fromIndex < toIndex) {
    setSelectedIndex(selectedFileIndex - 1);
  } else if (selectedFileIndex === toIndex && fromIndex > toIndex) {
    setSelectedIndex(selectedFileIndex + 1);
  } else if (selectedFileIndex > fromIndex && selectedFileIndex <= toIndex) {
    setSelectedIndex(selectedFileIndex - 1);
  } else if (selectedFileIndex < fromIndex && selectedFileIndex >= toIndex) {
    setSelectedIndex(selectedFileIndex + 1);
  }

  updateFileListDOM();
  onFileListChange();

  // Re-setup drag handlers for new order
  setupDragStartHandlers();
}

// Initialize event handlers for sort and clear buttons on DOM load
export function initDOMEventHandlers(): void {
  // No-op: handlers are bound in `index.ts` on DOMContentLoaded.
}
