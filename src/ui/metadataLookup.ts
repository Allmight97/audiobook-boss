import { bridge } from "../lib/bridge";
import type { AudioFile } from "../types/audio";
import type {
  AudiobookMetadata,
  MetadataSource,
  OnlineMetadataResult,
} from "../types/metadata";
import { applyMetadataToForm } from "./metadataForm";
import { onMetadataChange } from "./outputPanel";
import { updateTagPreview } from "./tagPreview";
import { setCustomCoverArt } from "./coverArt";
import { getMetadataForFile } from "./metadataState";
import { currentFileList } from "./fileList";
import { stageMetadataToSelection, selectFile } from "./fileList/actions";
import { getSelectedFileIndices } from "./fileList/state";

const DEFAULT_SOURCES: MetadataSource[] = ["open_library", "itunes"];

type ApplyMode = "current" | "all" | "queue";

type LookupQueueItem = {
  file: AudioFile;
  index: number;
};

let lookupQueue: LookupQueueItem[] = [];
let queueIndex = 0;
let currentResults: OnlineMetadataResult[] = [];

function getModal(): HTMLElement | null {
  return document.getElementById("metadata-lookup-modal");
}

function getResultsContainer(): HTMLElement | null {
  return document.getElementById("metadata-lookup-results");
}

function getSearchInput(): HTMLInputElement | null {
  const el = document.getElementById("metadata-lookup-query");
  return el instanceof HTMLInputElement ? el : null;
}

function getSourceSelect(): HTMLSelectElement | null {
  const el = document.getElementById("metadata-lookup-source");
  return el instanceof HTMLSelectElement ? el : null;
}

function getApplyModeSelect(): HTMLSelectElement | null {
  const el = document.getElementById("metadata-lookup-apply-mode");
  return el instanceof HTMLSelectElement ? el : null;
}

function getStatusEl(): HTMLElement | null {
  return document.getElementById("metadata-lookup-status");
}

function getQueueContextEl(): HTMLElement | null {
  return document.getElementById("metadata-lookup-context");
}

function setStatus(message: string, variant: "error" | "success" | "info" = "info"): void {
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
}

function hideModal(): void {
  const modal = getModal();
  if (!modal) return;
  modal.classList.remove("open");
}

function formatFileName(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}

function deriveQueryFromFile(file: AudioFile): string {
  const stored = getMetadataForFile(file.path) ?? {};
  const parts: string[] = [];
  if (stored.title) parts.push(stored.title);
  if (stored.artist) parts.push(stored.artist);
  if (stored.composer) parts.push(stored.composer);
  if (parts.length > 0) return parts.join(" ");

  const rawName = formatFileName(file.path).replace(/\.[^.]+$/, "");
  return rawName.replace(/[._-]+/g, " ").trim();
}

function updateQueueContext(): void {
  const contextEl = getQueueContextEl();
  if (!contextEl) return;
  if (lookupQueue.length === 0) {
    contextEl.textContent = "No files selected.";
    return;
  }

  const current = lookupQueue[queueIndex];
  const label = `${queueIndex + 1} of ${lookupQueue.length}`;
  contextEl.textContent = `${label} • ${formatFileName(current.file.path)}`;
}

function updateApplyModeOptions(): void {
  const applySelect = getApplyModeSelect();
  if (!applySelect) return;

  const multi = lookupQueue.length > 1;
  applySelect.innerHTML = "";

  const currentOption = document.createElement("option");
  currentOption.value = "current";
  currentOption.textContent = multi ? "Apply to current file" : "Apply to file";
  applySelect.appendChild(currentOption);

  if (multi) {
    const allOption = document.createElement("option");
    allOption.value = "all";
    allOption.textContent = "Apply to all selected";
    applySelect.appendChild(allOption);

    const queueOption = document.createElement("option");
    queueOption.value = "queue";
    queueOption.textContent = "Apply & next in queue";
    applySelect.appendChild(queueOption);

    applySelect.value = "all";
  } else {
    applySelect.value = "current";
  }
}

function getApplyMode(): ApplyMode {
  const select = getApplyModeSelect();
  if (!select) return "current";
  if (select.value === "all") return "all";
  if (select.value === "queue") return "queue";
  return "current";
}

function resetResults(): void {
  currentResults = [];
  const container = getResultsContainer();
  if (container) {
    container.replaceChildren();
  }
}

function renderResults(results: OnlineMetadataResult[]): void {
  const container = getResultsContainer();
  if (!container) return;
  container.replaceChildren();
  currentResults = results;

  if (results.length === 0) {
    const empty = document.createElement("div");
    empty.className = "metadata-lookup-empty muted-text";
    empty.textContent = "No matches found. Try another search.";
    container.appendChild(empty);
    return;
  }

  results.forEach((result, index) => {
    const row = document.createElement("div");
    row.className = "metadata-lookup-result";

    const cover = document.createElement("div");
    cover.className = "metadata-lookup-cover";
    if (result.coverUrl) {
      const img = document.createElement("img");
      img.src = result.coverUrl;
      img.alt = `${result.title} cover art`;
      img.loading = "lazy";
      cover.appendChild(img);
    } else {
      const placeholder = document.createElement("span");
      placeholder.textContent = "No Art";
      cover.appendChild(placeholder);
    }

    const details = document.createElement("div");
    details.className = "metadata-lookup-details";

    const title = document.createElement("div");
    title.className = "metadata-lookup-title";
    title.textContent = result.title;

    const authors = document.createElement("div");
    authors.className = "metadata-lookup-meta";
    authors.textContent = result.authors.length
      ? `Author: ${result.authors.join(", ")}`
      : "Author: —";

    const narrators = document.createElement("div");
    narrators.className = "metadata-lookup-meta";
    narrators.textContent = result.narrators.length
      ? `Narrator: ${result.narrators.join(", ")}`
      : "Narrator: —";

    const extra = document.createElement("div");
    extra.className = "metadata-lookup-meta";
    const year = result.publishedYear ? result.publishedYear.toString() : "—";
    const duration = result.durationSeconds
      ? `${Math.round(result.durationSeconds / 3600)}h`
      : "—";
    extra.textContent = `Year: ${year} • Length: ${duration}`;

    const source = document.createElement("span");
    source.className = "metadata-lookup-source";
    source.textContent = result.source === "itunes" ? "Apple Books" : "Open Library";

    details.appendChild(title);
    details.appendChild(authors);
    details.appendChild(narrators);
    details.appendChild(extra);
    details.appendChild(source);

    const applyButton = document.createElement("button");
    applyButton.type = "button";
    applyButton.className = "btn-pill btn-pill-secondary";
    applyButton.textContent = "Use Metadata";
    applyButton.dataset.index = index.toString();

    const actions = document.createElement("div");
    actions.className = "metadata-lookup-actions";
    actions.appendChild(applyButton);

    row.appendChild(cover);
    row.appendChild(details);
    row.appendChild(actions);

    container.appendChild(row);
  });
}

function mapResultToMetadata(result: OnlineMetadataResult): Partial<AudiobookMetadata> {
  const metadata: Partial<AudiobookMetadata> = {
    title: result.title,
  };

  if (result.authors.length > 0) {
    metadata.artist = result.authors.join(", ");
  }
  if (result.narrators.length > 0) {
    metadata.composer = result.narrators.join(", ");
  }
  if (result.series) {
    metadata.series = result.series;
  }
  if (result.seriesPart) {
    metadata.series_part = result.seriesPart;
  }
  if (result.description) {
    metadata.description = result.description;
  }
  if (result.publishedYear) {
    metadata.date = result.publishedYear;
  }
  if (result.title) {
    metadata.album = result.title;
  }

  return metadata;
}

async function applyCoverArt(result: OnlineMetadataResult): Promise<void> {
  if (!result.coverUrl) return;
  try {
    const coverBytes = await bridge.invoke<number[]>(
      "load_cover_art_from_url",
      { url: result.coverUrl }
    );
    setCustomCoverArt(coverBytes);
  } catch (error) {
    console.warn("Failed to load cover art from lookup:", error);
    setStatus("Cover art failed to load from source.", "error");
  }
}

async function applyResult(result: OnlineMetadataResult): Promise<void> {
  if (lookupQueue.length === 0) {
    setStatus("Select at least one file before applying metadata.", "error");
    return;
  }

  const metadata = mapResultToMetadata(result);
  const mode = getApplyMode();

  if (mode === "all") {
    applyMetadataToForm(metadata, { mode: "multi", markDirty: true });
    await applyCoverArt(result);
    await stageMetadataToSelection({ showStatus: true });
    setStatus("Metadata staged for all selected files.", "success");
    return;
  }

  const current = lookupQueue[queueIndex];
  if (current) {
    await selectFile(current.index, { multi: false, range: false });
  }

  applyMetadataToForm(metadata, { mode: "single", markDirty: true });
  await applyCoverArt(result);
  onMetadataChange();
  updateTagPreview();
  setStatus("Metadata applied to form.", "success");

  if (mode === "queue" && queueIndex < lookupQueue.length - 1) {
    queueIndex += 1;
    updateQueueContext();
    const nextItem = lookupQueue[queueIndex];
    if (nextItem) {
      await selectFile(nextItem.index, { multi: false, range: false });
    }
    const searchInput = getSearchInput();
    if (searchInput) {
      searchInput.value = deriveQueryFromFile(lookupQueue[queueIndex].file);
    }
    resetResults();
  }
}

async function runSearch(): Promise<void> {
  const queryInput = getSearchInput();
  const sourceSelect = getSourceSelect();
  if (!queryInput || !sourceSelect) return;

  const query = queryInput.value.trim();
  if (!query) {
    setStatus("Enter a title or author to search.", "error");
    return;
  }

  const selectedSource = sourceSelect.value as MetadataSource | "all";
  const sources = selectedSource === "all" ? DEFAULT_SOURCES : [selectedSource];

  setStatus("Searching metadata sources…", "info");

  try {
    const results = await bridge.invoke<OnlineMetadataResult[]>(
      "search_online_metadata",
      { query, sources, limit: 8 }
    );
    renderResults(results);
    setStatus(`Found ${results.length} results.`, "success");
  } catch (error) {
    console.error("Metadata lookup failed:", error);
    setStatus("Search failed. Check your query and try again.", "error");
  }
}

function openLookup(): void {
  const selectedIndices = Array.from(getSelectedFileIndices()).sort((a, b) => a - b);
  lookupQueue = selectedIndices
    .map((index) => {
      const file = currentFileList?.files[index];
      if (!file || !file.isValid) return null;
      return { file, index };
    })
    .filter((item): item is LookupQueueItem => Boolean(item));
  queueIndex = 0;

  if (lookupQueue.length === 0) {
    setStatus("Select a valid file to search metadata.", "error");
  } else {
    const queryInput = getSearchInput();
    if (queryInput) {
      queryInput.value = deriveQueryFromFile(lookupQueue[0].file);
    }
    setStatus("", "info");
  }

  updateQueueContext();
  updateApplyModeOptions();
  resetResults();
  showModal();
}

export function initMetadataLookup(): void {
  const openButton = document.getElementById(
    "metadata-lookup-btn"
  ) as HTMLButtonElement | null;
  const closeButton = document.getElementById(
    "metadata-lookup-close"
  ) as HTMLButtonElement | null;
  const searchButton = document.getElementById(
    "metadata-lookup-search-btn"
  ) as HTMLButtonElement | null;
  const modal = getModal();

  if (openButton) {
    openButton.addEventListener("click", openLookup);
  }

  if (closeButton) {
    closeButton.addEventListener("click", hideModal);
  }

  if (modal) {
    modal.addEventListener("click", (event) => {
      if (event.target === modal) {
        hideModal();
      }
    });
  }

  if (searchButton) {
    searchButton.addEventListener("click", () => {
      void runSearch();
    });
  }

  const searchInput = getSearchInput();
  if (searchInput) {
    searchInput.addEventListener("keydown", (event) => {
      if (event.key === "Enter") {
        event.preventDefault();
        void runSearch();
      }
    });
  }

  const results = getResultsContainer();
  if (results) {
    results.addEventListener("click", (event) => {
      const target = event.target as HTMLElement;
      const button = target.closest<HTMLButtonElement>("button[data-index]");
      if (!button) return;
      const index = Number(button.dataset.index ?? "-1");
      const result = currentResults[index];
      if (!result) return;
      void applyResult(result);
    });
  }
}
