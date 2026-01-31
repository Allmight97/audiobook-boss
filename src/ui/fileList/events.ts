import {
  currentFileList,
  isOrderLocked,
} from "./state";
import { isMetadataSaveInProgress } from "../metadataSaveState";
import {
  selectFile,
  removeFile,
  moveFileUp,
  moveFileDown,
  reorderFiles,
  selectAll,
  clearSelectionAction,
} from "./actions";

let draggedIndex: number | null = null;

export function initFileListEvents(): void {
  const container = document.querySelector<HTMLElement>(".file-list-content");
  if (!container) return;

  container.removeEventListener("click", handleFileListClick);
  container.removeEventListener("dragover", handleDragOver);
  container.removeEventListener("drop", handleDrop);
  container.removeEventListener("dragend", handleDragEnd);

  container.addEventListener("click", handleFileListClick);
  container.addEventListener("dragover", handleDragOver);
  container.addEventListener("drop", handleDrop);
  container.addEventListener("dragend", handleDragEnd);

  document.removeEventListener("keydown", handleFileListKeyDown);
  document.addEventListener("keydown", handleFileListKeyDown);

  setupDragStartHandlers();
}

export function setupDragStartHandlers(): void {
  const container = document.querySelector<HTMLElement>(".file-list-content");
  if (!container) return;

  const items = container.querySelectorAll<HTMLElement>(".file-list-item");
  items.forEach((item, index) => {
    const existingHandler = (item as any).__dragStartHandler;
    if (existingHandler) {
      item.removeEventListener("dragstart", existingHandler);
    }

    const handler = (e: DragEvent) => handleDragStart(e, index);
    (item as any).__dragStartHandler = handler;
    item.addEventListener("dragstart", handler);
  });
}

function handleFileListClick(e: Event): void {
  if (isMetadataSaveInProgress()) return;
  const target = e.target as HTMLElement;

  if (target.classList.contains("remove-file-btn")) {
    e.stopPropagation();
    e.preventDefault();
    if (isOrderLocked()) return;
    const index = parseInt(target.dataset.index || "-1");
    if (index >= 0) {
      removeFile(index);
    }
    return;
  }

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

  if (target.classList.contains("move-down-btn")) {
    e.stopPropagation();
    e.preventDefault();
    if (isOrderLocked()) return;
    const index = parseInt(target.dataset.index || "-1");
    if (index >= 0 && currentFileList && index < currentFileList.files.length - 1) {
      moveFileDown(index);
    }
    return;
  }

  const fileItem = target.closest(".file-list-item") as HTMLElement;
  if (fileItem) {
    const index = parseInt(fileItem.dataset.index || "-1");
    if (index >= 0) {
      const mouseEvent = e as MouseEvent;
      const multi = mouseEvent.ctrlKey || mouseEvent.metaKey;
      const range = mouseEvent.shiftKey;

      if (range) {
        window.getSelection()?.removeAllRanges();
      }

      void selectFile(index, { multi, range });
    }
  }
}

function handleFileListKeyDown(e: KeyboardEvent): void {
  if (isMetadataSaveInProgress()) return;
  if (!currentFileList) return;
  if (isTextInputTarget(e.target)) return;

  const key = e.key.toLowerCase();
  if ((e.metaKey || e.ctrlKey) && key === "a") {
    e.preventDefault();
    selectAll();
    return;
  }

  if (key === "escape") {
    e.preventDefault();
    void clearSelectionAction();
  }
}

function isTextInputTarget(target: EventTarget | null): boolean {
  if (!target || !(target instanceof HTMLElement)) return false;
  const tagName = target.tagName.toLowerCase();
  return tagName === "input" || tagName === "textarea";
}

function handleDragStart(e: DragEvent, index: number): void {
  if (isMetadataSaveInProgress()) return;
  if (isOrderLocked()) return;
  if (!e.dataTransfer) return;

  draggedIndex = index;
  e.dataTransfer.effectAllowed = "move";
  e.dataTransfer.setData("text/plain", index.toString());

  const item = e.currentTarget as HTMLElement;
  item.classList.add("dragging");
}

function handleDragOver(e: DragEvent): void {
  if (isMetadataSaveInProgress()) return;
  if (isOrderLocked()) return;
  e.preventDefault();
  if (!e.dataTransfer) return;

  e.dataTransfer.dropEffect = "move";

  const container = e.currentTarget as HTMLElement;
  const items = Array.from(container.querySelectorAll(".file-list-item"));

  items.forEach((item) => item.classList.remove("drag-over"));

  const target = e.target as HTMLElement;
  const fileItem = target.closest(".file-list-item") as HTMLElement;
  if (fileItem && !fileItem.classList.contains("dragging")) {
    fileItem.classList.add("drag-over");
  }
}

function handleDrop(e: DragEvent): void {
  if (isMetadataSaveInProgress()) return;
  if (isOrderLocked()) return;
  e.preventDefault();
  e.stopPropagation();

  if (draggedIndex === null) return;

  const container = e.currentTarget as HTMLElement;
  const items = Array.from(container.querySelectorAll(".file-list-item"));

  items.forEach((item) => item.classList.remove("drag-over"));

  const target = e.target as HTMLElement;
  const dropTarget = target.closest(".file-list-item") as HTMLElement;
  if (!dropTarget) return;

  const dropIndex = parseInt(dropTarget.dataset.index || "-1");
  if (dropIndex < 0 || dropIndex === draggedIndex) return;

  reorderFiles(draggedIndex, dropIndex);

  draggedIndex = null;
}

function handleDragEnd(e: DragEvent): void {
  const container = e.currentTarget as HTMLElement;
  const draggedItem = container.querySelector(".file-list-item.dragging");
  if (draggedItem) {
    draggedItem.classList.remove("dragging");
  }

  container.querySelectorAll(".file-list-item").forEach((item) => {
    item.classList.remove("drag-over");
  });

  draggedIndex = null;
}

// Initialize event handlers for sort and clear buttons on DOM load
export function initDOMEventHandlers(): void {
  // No-op: handlers are bound in `index.ts` on DOMContentLoaded.
}
