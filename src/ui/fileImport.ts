import { bridge } from "../lib/bridge";
import { FileListInfo } from "../types/audio";
import { isFileDropEvent } from "../types/events";
import { mount, unmount } from "svelte";
import { displayFileList } from "./fileList";
import FileImportIsland from "./fileImport/FileImportIsland.svelte";
import { isOrderLocked } from "./fileList/state";

const FILE_IMPORT_ROOT_ID = "file-import-root";

let dropZoneHeader: HTMLElement | null = null;
let mountedFileImportRoot: HTMLElement | null = null;
let mountedFileImportIsland: Parameters<typeof unmount>[0] | null = null;

export function initFileImport(): void {
  mountFileImportIsland();

  dropZoneHeader = document.querySelector(".drop-zone-header");
  if (!dropZoneHeader) return;

  setupDragDropHandlers();
  setupClickToSelect();
  setupKeyboardHandler();
  updateDropZoneState(false);
}

function mountFileImportIsland(): void {
  const importRoot = document.getElementById(FILE_IMPORT_ROOT_ID);
  if (!importRoot) return;

  if (
    mountedFileImportIsland &&
    mountedFileImportRoot === importRoot &&
    importRoot.childElementCount > 0
  ) {
    return;
  }

  if (mountedFileImportIsland) {
    void unmount(mountedFileImportIsland);
    mountedFileImportIsland = null;
  }

  mountedFileImportIsland = mount(FileImportIsland, { target: importRoot });
  mountedFileImportRoot = importRoot;
}

function setupDragDropHandlers(): void {
  if (!dropZoneHeader) return;

  // Listen for the Tauri file drop event
  // Payload is now { paths: string[], position: { x, y } }
  bridge.listen(
    "tauri://drag-drop",
    async (event) => {
      dropZoneHeader?.classList.remove("drag-over");
      if (isFileDropEvent(event.payload)) {
        // Ignore drops that target the cover art area to prevent importing images as audio
        const coverArea = document.getElementById("cover-art-area");
        if (coverArea) {
          const rect = coverArea.getBoundingClientRect();
          const { x, y } = event.payload.position;
          if (
            x >= rect.left &&
            x <= rect.right &&
            y >= rect.top &&
            y <= rect.bottom
          ) {
            return;
          }
        }

        // Accept drops anywhere on the file management container (not just header)
        // This allows dropping files on the file list area when files are present
        const fileManagementContainer = document.querySelector(
          ".file-management-container"
        );
        if (fileManagementContainer) {
          const rect = fileManagementContainer.getBoundingClientRect();
          const { x, y } = event.payload.position;
          if (
            x >= rect.left &&
            x <= rect.right &&
            y >= rect.top &&
            y <= rect.bottom
          ) {
            await handleFileDrop(event.payload.paths);
          }
        }
      }
    }
  );

  bridge.listen("tauri://drag-enter", () => {
    // Add drag-over class to header for visual feedback
    dropZoneHeader?.classList.add("drag-over");
  });

  bridge.listen("tauri://drag-leave", () => {
    dropZoneHeader?.classList.remove("drag-over");
  });
}

function setupClickToSelect(): void {
  if (!dropZoneHeader) return;

  dropZoneHeader.addEventListener("click", handleClickToSelect);
}

function setupKeyboardHandler(): void {
  if (!dropZoneHeader) return;

  dropZoneHeader.addEventListener("keydown", (e) => {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      handleClickToSelect();
    }
  });
}

export function updateDropZoneState(hasFiles: boolean): void {
  if (dropZoneHeader) {
    dropZoneHeader.setAttribute("data-has-files", hasFiles.toString());
  }
}

async function handleFileDrop(paths: string[]): Promise<void> {
  if (isOrderLocked()) {
    showError("Order locked while processing. Wait for completion to add files.");
    return;
  }
  const supportedPaths = filterSupportedFiles(paths);
  if (supportedPaths.length === 0) {
    showError(
      "No supported audio files dropped. Please use MP3, M4A, M4B, or AAC files."
    );
    return;
  }
  await processFilePaths(supportedPaths);
}

async function handleClickToSelect(): Promise<void> {
  if (isOrderLocked()) {
    showError("Order locked while processing. Wait for completion to add files.");
    return;
  }
  try {
    const selected = await bridge.open({
      multiple: true,
      directory: false,
      filters: [
        {
          name: "Audio Files",
          extensions: ["mp3", "m4a", "m4b", "aac"],
        },
      ],
    });

    if (Array.isArray(selected) && selected.length > 0) {
      await processFilePaths(selected);
    } else if (typeof selected === "string") {
      await processFilePaths([selected]);
    }
  } catch (error) {
    showError(`Failed to bridge.open file dialog: ${error}`);
  }
}

function filterSupportedFiles(paths: string[]): string[] {
  const supportedFormats = [".mp3", ".m4a", ".m4b", ".aac"];
  return paths.filter((path) =>
    supportedFormats.some((format) => path.toLowerCase().endsWith(format))
  );
}

async function processFilePaths(filePaths: string[]): Promise<void> {
  if (filePaths.length === 0) return;

  try {
    const fileListInfo: FileListInfo = await bridge.analyzeAudioFiles(filePaths);
    displayFileList(fileListInfo);
    clearError();
  } catch (error) {
    showError(`Failed to analyze files: ${error}`);
  }
}

function showError(message: string): void {
  const errorElement = document.getElementById("file-import-error");
  if (errorElement) {
    errorElement.textContent = message;
    errorElement.style.display = "block";
  }
}

function clearError(): void {
  const errorElement = document.getElementById("file-import-error");
  if (errorElement) {
    errorElement.style.display = "none";
  }
}
