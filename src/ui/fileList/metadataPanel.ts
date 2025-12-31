import { AudioFile, formatFileSize } from "../../types/audio";
import { bridge } from "../../lib/bridge";
import type { AudiobookMetadata } from "../../types/metadata";
import { onMetadataChange } from "../outputPanel";
import { updateTagPreview } from "../tagPreview";
import {
  clearCoverArt,
  getHasCustomCoverArt,
  setCoverArt,
} from "../coverArt";
import {
  getMetadataForFile,
  setMetadataForFile,
} from "../metadataState";
import {
  populateMetadataFormMulti,
  populateMetadataFormSingle,
  resetDirtyState,
} from "../metadataForm";
import {
  currentFileList,
  getSelectedFileIndices,
  selectedFileIndex,
} from "./state";

function setText(id: string, value: string): void {
  const el = document.getElementById(id);
  if (el) el.textContent = value;
}

function updatePropertiesContextSingle(file: AudioFile, index: number): void {
  const contextEl = document.getElementById("prop-selected-context");
  if (!contextEl || !currentFileList) return;

  contextEl.replaceChildren();

  if (index < 0 || index >= currentFileList.files.length) {
    const emptySpan = document.createElement("span");
    emptySpan.className = "context-empty";
    emptySpan.textContent = "No file selected";
    contextEl.appendChild(emptySpan);
    return;
  }

  const fileName = file.path.split(/[\\/]/).pop() || file.path;
  const totalFiles = currentFileList.files.length;

  const filenameSpan = document.createElement("span");
  filenameSpan.className = "context-filename";
  filenameSpan.title = fileName;
  filenameSpan.textContent = fileName;

  const posSpan = document.createElement("span");
  posSpan.className = "context-position";
  posSpan.textContent = `${index + 1} of ${totalFiles}`;

  contextEl.appendChild(filenameSpan);
  contextEl.appendChild(posSpan);
}

function updatePropertiesContextMulti(selectedCount: number): void {
  const contextEl = document.getElementById("prop-selected-context");
  if (!contextEl) return;

  contextEl.replaceChildren();

  const span = document.createElement("span");
  span.className = "context-filename";
  span.textContent = `${selectedCount} files selected`;
  contextEl.appendChild(span);
}

function clearPropertiesContext(): void {
  const contextEl = document.getElementById("prop-selected-context");
  if (!contextEl) return;

  contextEl.replaceChildren();
  const emptySpan = document.createElement("span");
  emptySpan.className = "context-empty";
  emptySpan.textContent = "No file selected";
  contextEl.appendChild(emptySpan);
}

function clearPropertyValues(): void {
  setText("prop-bitrate", "---");
  setText("prop-samplerate", "---");
  setText("prop-channels", "---");
  setText("prop-filesize", "---");
}

async function loadMetadataForFile(
  file: AudioFile
): Promise<Partial<AudiobookMetadata> | null> {
  if (!file.isValid) return null;

  const existing = getMetadataForFile(file.path);
  if (existing) return existing;

  try {
    const metadata = await bridge.invoke<AudiobookMetadata>(
      "read_audio_metadata",
      { filePath: file.path }
    );
    setMetadataForFile(file.path, metadata);
    return metadata;
  } catch (error) {
    console.warn("Failed to load metadata:", error);
    return null;
  }
}

export async function ensureMetadataForFiles(
  files: AudioFile[]
): Promise<void> {
  const validFiles = files.filter((file) => file.isValid);
  await Promise.all(validFiles.map((file) => loadMetadataForFile(file)));
}

export function updateFileProperties(
  file: AudioFile,
  options?: { skipMetadataLoad?: boolean }
): void {
  if (file.isValid) {
    setText("prop-bitrate", file.bitrate ? `${file.bitrate} kb/s` : "N/A");
    setText(
      "prop-samplerate",
      file.sampleRate ? `${file.sampleRate} Hz` : "N/A"
    );
    setText("prop-channels", file.channels ? `${file.channels} ch` : "N/A");
    setText("prop-filesize", file.size ? formatFileSize(file.size) : "N/A");

    if (!options?.skipMetadataLoad) {
      void loadMetadataForFile(file).then((metadata) => {
        if (metadata) {
          populateMetadataFormSingle(metadata);
          onMetadataChange();
          updateTagPreview();
        }
      });
    }
  } else {
    clearPropertyValues();
  }

  updatePropertiesContextSingle(file, selectedFileIndex);
}

export async function showSingleSelection(file: AudioFile): Promise<void> {
  const stored = getMetadataForFile(file.path);
  if (stored) {
    updateFileProperties(file, { skipMetadataLoad: true });
    populateMetadataFormSingle(stored);
  } else {
    updateFileProperties(file);
  }

  onMetadataChange();
  updateTagPreview();
}

export async function showMultiSelection(
  selectedFiles: AudioFile[]
): Promise<void> {
  const selectedCount = selectedFiles.length;

  updatePropertiesContextMulti(selectedCount);
  clearPropertyValues();

  resetDirtyState();

  const validFiles = selectedFiles.filter((file) => file.isValid);
  const metadataList = await Promise.all(
    validFiles.map(async (file) => {
      const metadata = await loadMetadataForFile(file);
      return metadata ?? {};
    })
  );

  populateMetadataFormMulti(metadataList, selectedCount);
  onMetadataChange();
  updateTagPreview();
}

export function clearSelectionPanels(): void {
  clearPropertyValues();
  clearPropertiesContext();
  populateMetadataFormSingle({});
  clearCoverArt();
}

export async function autoUpdateCoverArtFromFirstValidFile(): Promise<void> {
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
    setCoverArt(null);
    console.warn("Failed to auto-load cover art:", error);
  }
}

export function getSelectedFiles(): AudioFile[] {
  const fileList = currentFileList;
  if (!fileList) return [];
  const selectedIndices = getSelectedFileIndices();
  return Array.from(selectedIndices)
    .map((index) => fileList.files[index])
    .filter((file): file is AudioFile => Boolean(file));
}
