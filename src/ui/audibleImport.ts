import { bridge } from "../lib/bridge";
import type { FileListInfo } from "../types/audio";
import type { AudibleImportPayload } from "../types/audible";
import { displayFileList } from "./fileList";
import { isOrderLocked } from "./fileList/state";

type StatusVariant = "error" | "success" | "info";

const AUDIBLE_EXTENSIONS = ["aax"];
const DEFAULT_STATUS = "Select .aax files and provide activation bytes.";

let selectedFiles: string[] = [];

function getModal(): HTMLElement | null {
  return document.getElementById("audible-import-modal");
}

function getOpenButton(): HTMLButtonElement | null {
  const el = document.getElementById("audible-import-open");
  return el instanceof HTMLButtonElement ? el : null;
}

function getCloseButton(): HTMLButtonElement | null {
  const el = document.getElementById("audible-import-close");
  return el instanceof HTMLButtonElement ? el : null;
}

function getSelectButton(): HTMLButtonElement | null {
  const el = document.getElementById("audible-import-select");
  return el instanceof HTMLButtonElement ? el : null;
}

function getStartButton(): HTMLButtonElement | null {
  const el = document.getElementById("audible-import-start");
  return el instanceof HTMLButtonElement ? el : null;
}

function getFilesInput(): HTMLInputElement | null {
  const el = document.getElementById("audible-import-files");
  return el instanceof HTMLInputElement ? el : null;
}

function getActivationInput(): HTMLInputElement | null {
  const el = document.getElementById("audible-import-activation");
  return el instanceof HTMLInputElement ? el : null;
}

function getRetainToggle(): HTMLInputElement | null {
  const el = document.getElementById("audible-import-retain");
  return el instanceof HTMLInputElement ? el : null;
}

function getStatusEl(): HTMLElement | null {
  return document.getElementById("audible-import-status");
}

function setStatus(message: string, variant: StatusVariant = "info"): void {
  const statusEl = getStatusEl();
  if (!statusEl) return;
  statusEl.textContent = message;
  statusEl.classList.toggle("is-error", variant === "error");
  statusEl.classList.toggle("is-success", variant === "success");
}

function showModal(): void {
  const modal = getModal();
  if (!modal) return;
  modal.classList.add("open");
  modal.setAttribute("aria-hidden", "false");
  setStatus(DEFAULT_STATUS, "info");
}

function hideModal(): void {
  const modal = getModal();
  if (!modal) return;
  modal.classList.remove("open");
  modal.setAttribute("aria-hidden", "true");
}

function updateFilesSummary(): void {
  const input = getFilesInput();
  if (!input) return;
  if (selectedFiles.length === 0) {
    input.value = "";
    input.placeholder = "No files selected";
    return;
  }
  input.value = `${selectedFiles.length} file${selectedFiles.length === 1 ? "" : "s"} selected`;
}

function normalizeActivationBytes(raw: string): string | null {
  const trimmed = raw.trim().replace(/\s+/g, "");
  const cleaned = trimmed.startsWith("0x") ? trimmed.slice(2) : trimmed;
  if (!/^[0-9a-fA-F]+$/.test(cleaned)) return null;
  if (cleaned.length !== 16) return null;
  return cleaned.toLowerCase();
}

async function selectFiles(): Promise<void> {
  try {
    const selected = await bridge.open({
      multiple: true,
      directory: false,
      filters: [
        {
          name: "Audible Downloads",
          extensions: AUDIBLE_EXTENSIONS,
        },
      ],
    });

    if (Array.isArray(selected) && selected.length > 0) {
      selectedFiles = selected;
    } else if (typeof selected === "string") {
      selectedFiles = [selected];
    }

    updateFilesSummary();
    if (selectedFiles.length > 0) {
      setStatus("Files ready. Provide activation bytes to continue.", "info");
    } else {
      setStatus(DEFAULT_STATUS, "info");
    }
  } catch (error) {
    setStatus(`Failed to open file dialog: ${error}`, "error");
  }
}

async function startImport(): Promise<void> {
  if (isOrderLocked()) {
    setStatus("Order locked while processing. Wait for completion.", "error");
    return;
  }

  if (selectedFiles.length === 0) {
    setStatus("Select at least one .aax file first.", "error");
    return;
  }

  const activationInput = getActivationInput();
  const activationRaw = activationInput?.value ?? "";
  const activationBytes = normalizeActivationBytes(activationRaw);
  if (!activationBytes) {
    setStatus("Activation bytes must be 16 hex characters.", "error");
    return;
  }

  const retain = getRetainToggle()?.checked ?? false;
  const payload: AudibleImportPayload = {
    filePaths: selectedFiles,
    activationBytes,
    retainOriginal: retain,
  };

  try {
    setStatus("Decrypting Audible downloads...", "info");
    const fileListInfo: FileListInfo = await bridge.invoke(
      "decrypt_audible_titles",
      payload
    );
    displayFileList(fileListInfo);
    setStatus("Audible titles decrypted and loaded.", "success");
    resetForm();
    hideModal();
  } catch (error) {
    setStatus(`Audible import failed: ${error}`, "error");
  }
}

function resetForm(): void {
  selectedFiles = [];
  updateFilesSummary();
  const activationInput = getActivationInput();
  if (activationInput) {
    activationInput.value = "";
  }
  const retainToggle = getRetainToggle();
  if (retainToggle) {
    retainToggle.checked = false;
  }
}

export function initAudibleImport(): void {
  const openButton = getOpenButton();
  const closeButton = getCloseButton();
  const selectButton = getSelectButton();
  const startButton = getStartButton();

  if (!openButton || !closeButton || !selectButton || !startButton) return;

  openButton.addEventListener("click", () => {
    resetForm();
    showModal();
  });

  closeButton.addEventListener("click", () => {
    hideModal();
  });

  selectButton.addEventListener("click", () => {
    void selectFiles();
  });

  startButton.addEventListener("click", () => {
    void startImport();
  });
}
