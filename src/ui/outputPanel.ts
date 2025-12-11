// import { invoke } from "@tauri-apps/api/core";
import { bridge } from "../lib/bridge";
import type {
  EncoderSettings,
  EncoderChannelConfig,
  SampleRateConfig,
  OutputConfig,
} from "../types/audio";
import type { AudiobookMetadata } from "../types/metadata";
import { currentFileList } from "./fileList";
import { formatFileSize, defaultEncoderSettings } from "../types/audio";
import { getCurrentCoverArt } from "./coverArt";
import { getJobType } from "./jobControls";
import {
  toBoundaryEncoderSettings,
  EncoderSettingsLike,
} from "../types/encoder";

type WindowWithEncoderProvider = Window & {
  EncoderSettingsProvider?: () => EncoderSettingsLike;
};

interface OutputPanelState {
  encoderSettings: EncoderSettings;
  sampleRate: SampleRateConfig;
  outputDirectory: string;
  useSubdirPattern: boolean;
  filenamePattern: "title_year" | "author_title";
}

let currentState: OutputPanelState = {
  encoderSettings: { ...defaultEncoderSettings() },
  sampleRate: { explicit: 22050 },
  outputDirectory: "",
  useSubdirPattern: true,
  filenamePattern: "title_year",
};

/**
 * Initializes the output panel with event handlers
 */
export function initOutputPanel(): void {
  setupEventHandlers();
  loadInitialState();
  updateOutputPath();
  updateEstimatedSize();
}

/**
 * Sets up all event handlers for output settings controls
 */
function setupEventHandlers(): void {
  setupSettingsHandlers();
  setupDirectoryHandlers();
  setupPatternHandlers();
  setupCollapsiblePanel();

  document.addEventListener("abb:job-type-changed", () => {
    updateOutputPath();
  });
}

function setupCollapsiblePanel(): void {
  const toggle = document.getElementById("path-options-toggle");
  const panel = document.getElementById("path-options-panel");
  if (toggle && panel) {
    toggle.addEventListener("click", () => {
      toggle.classList.toggle("expanded");
      panel.classList.toggle("expanded");
    });
  }
}

/**
 * Sets up audio settings event handlers
 */
function setupSettingsHandlers(): void {
  const bitrateSelect = document.getElementById(
    "output-bitrate"
  ) as HTMLSelectElement;
  const sampleRateSelect = document.getElementById(
    "output-samplerate"
  ) as HTMLSelectElement;
  const channelsSelect = document.getElementById(
    "output-channels"
  ) as HTMLSelectElement;

  if (bitrateSelect) {
    bitrateSelect.addEventListener("change", handleBitrateChange);
  }

  if (sampleRateSelect) {
    sampleRateSelect.addEventListener("change", handleSampleRateChange);
  }

  if (channelsSelect) {
    channelsSelect.addEventListener("change", handleChannelsChange);
  }
}

/**
 * Sets up directory selection event handlers
 */
function setupDirectoryHandlers(): void {
  const browseButton = document.getElementById(
    "output-dir-browse"
  ) as HTMLButtonElement;
  const subdirCheckbox = document.getElementById(
    "output-subdir-pattern"
  ) as HTMLInputElement;

  if (browseButton) {
    browseButton.addEventListener("click", handleDirectoryBrowse);
  }

  if (subdirCheckbox) {
    subdirCheckbox.addEventListener("change", handleSubdirPatternChange);
  }
}

/**
 * Sets up filename pattern event handlers
 */
function setupPatternHandlers(): void {
  const patternRadios = document.querySelectorAll(
    'input[name="filename_pattern"]'
  );
  patternRadios.forEach((radio) => {
    radio.addEventListener("change", handleFilenamePatternChange);
  });
}

/**
 * Handles bitrate selection change
 */
function handleBitrateChange(event: Event): void {
  const target = event.target as HTMLSelectElement;
  currentState.encoderSettings = {
    ...currentState.encoderSettings,
    bitrateKbps: parseInt(target.value) as EncoderSettings["bitrateKbps"],
  };
  updateEstimatedSize();
}

/**
 * Handles sample rate selection change
 */
function handleSampleRateChange(event: Event): void {
  const target = event.target as HTMLSelectElement;
  const value = target.value;
  currentState.sampleRate =
    value === "auto" ? "auto" : { explicit: parseInt(value) };
  updateEstimatedSize();
}

/**
 * Handles channel configuration change
 */
function handleChannelsChange(event: Event): void {
  const target = event.target as HTMLSelectElement;
  const channels: EncoderChannelConfig =
    target.value === "mono"
      ? "mono"
      : target.value === "stereo"
        ? "stereo"
        : "auto";
  currentState.encoderSettings = { ...currentState.encoderSettings, channels };
  updateEstimatedSize();
}

/**
 * Handles directory browse button click
 */
async function handleDirectoryBrowse(): Promise<void> {
  try {
    const selectedPath = await bridge.open({
      directory: true,
      multiple: false,
      title: "Select Output Directory",
    });

    if (selectedPath && typeof selectedPath === "string") {
      currentState.outputDirectory = selectedPath;
      updateOutputPath();
    }
  } catch (error) {
    console.error("Error selecting directory:", error);
    showOutputError("Failed to select directory");
  }
}

/**
 * Handles subdirectory pattern checkbox change
 */
function handleSubdirPatternChange(event: Event): void {
  const target = event.target as HTMLInputElement;
  currentState.useSubdirPattern = target.checked;
  updateOutputPath();
}

/**
 * Handles filename pattern radio button change
 */
function handleFilenamePatternChange(event: Event): void {
  const target = event.target as HTMLInputElement;
  currentState.filenamePattern = target.value as "title_year" | "author_title";
  updateOutputPath();
}

/**
 * Loads initial state from HTML elements
 */
function loadInitialState(): void {
  const bitrateSelect = document.getElementById(
    "output-bitrate"
  ) as HTMLSelectElement;
  const sampleRateSelect = document.getElementById(
    "output-samplerate"
  ) as HTMLSelectElement;
  const channelsSelect = document.getElementById(
    "output-channels"
  ) as HTMLSelectElement;

  if (bitrateSelect) {
    currentState.encoderSettings = {
      ...currentState.encoderSettings,
      bitrateKbps: parseInt(
        bitrateSelect.value
      ) as EncoderSettings["bitrateKbps"],
    };
  }

  if (sampleRateSelect) {
    const value = sampleRateSelect.value;
    currentState.sampleRate =
      value === "auto" ? "auto" : { explicit: parseInt(value) };
  }

  if (channelsSelect) {
    const channels: EncoderChannelConfig =
      channelsSelect.value === "mono"
        ? "mono"
        : channelsSelect.value === "stereo"
          ? "stereo"
          : "auto";
    currentState.encoderSettings = { ...currentState.encoderSettings, channels };
  }
}

/**
 * Updates the output path PREVIEW display
 */
function updateOutputPath(): void {
  const previewText = document.getElementById("output-preview-text");
  const outputPathInput = document.getElementById(
    "output-dir-text"
  ) as HTMLInputElement;

  // Basic validation state
  if (!currentState.outputDirectory) {
    if (previewText) previewText.textContent = "Select output directory...";
    if (previewText) previewText.title = "No directory selected";
    if (outputPathInput) outputPathInput.value = "";
    return;
  }

  // Update hidden input for reference
  if (outputPathInput) outputPathInput.value = currentState.outputDirectory;

  // Calculate generic preview (using First File or Placeholder)
  const metadata = getCurrentMetadata();
  const calculatedPath = calculateOutputPath(metadata);

  if (previewText) {
    previewText.textContent = calculatedPath;
    previewText.title = calculatedPath; // Tooltip for full path
  }
}

/**
 * Calculates the full output path based on current settings for PREVIEW
 */
function calculateOutputPath(metadata: AudiobookMetadata): string {
  let basePath = currentState.outputDirectory || "[Output Directory]";
  const jobType = getJobType();

  if (jobType === "batch") {
    basePath += "/(Batch Output Folder)";
    // In batch mode, we might just show a generic pattern hint
    return `${basePath}/[Author]/[Series]/[Title]/Title (Year).m4b`;
  }

  if (currentState.useSubdirPattern) {
    basePath = buildSubdirectoryPath(basePath, metadata);
  }

  const filename = buildFilename(metadata);
  return `${basePath}/${filename}`;
}

/**
 * Builds subdirectory pattern: [Author]/[Series]/[Title]
 */
function buildSubdirectoryPath(
  basePath: string,
  metadata: AudiobookMetadata
): string {
  const author = sanitizeFilename(metadata.author || "Unknown Author");
  const series = sanitizeFilename(metadata.series || "");
  const title = sanitizeFilename(metadata.title || "Untitled");

  let subdirPath = `${basePath}/${author}`;

  if (series) {
    subdirPath += `/${series}`;
  }

  subdirPath += `/${title}`;

  return subdirPath;
}

/**
 * Sanitizes a string for use in filenames by replacing problematic characters
 */
function sanitizeFilename(input: string): string {
  return input
    .replace(/[,]/g, "_")
    .replace(/[/\\:*?"<>|]/g, "_")
    .trim();
}

/**
 * Builds output filename based on pattern selection
 */
function buildFilename(metadata: AudiobookMetadata): string {
  const title = sanitizeFilename(metadata.title || "Untitled");
  const author = sanitizeFilename(metadata.author || "Unknown Author");
  const year = metadata.year || new Date().getFullYear();

  if (currentState.filenamePattern === "author_title") {
    return `${author} - ${title}.m4b`;
  }

  return `${title} (${year}).m4b`;
}

/**
 * Gets current metadata from the metadata panel
 */
function getCurrentMetadata(): AudiobookMetadata {
  const getElementValue = (id: string): string => {
    const element = document.getElementById(id) as HTMLInputElement;
    return element?.value || "";
  };

  const coverArt = getCurrentCoverArt();
  const title = getElementValue("meta-title");

  return {
    title: title,
    author: getElementValue("meta-author"),
    album: title,
    narrator: getElementValue("meta-narrator"),
    year: parseInt(getElementValue("meta-year")) || undefined,
    genre: getElementValue("meta-genre"),
    description: getElementValue("meta-description"),
    series: getElementValue("meta-series"),
    cover_art: coverArt ?? undefined,
  };
}

/**
 * Updates the estimated output size display
 */
function updateEstimatedSize(): void {
  const sizeElement = document.getElementById("estimated-size");
  if (!sizeElement) return;

  const fileList = currentFileList;
  if (!fileList || !fileList.files.length) {
    sizeElement.textContent = "~ --- MB";
    return;
  }

  const estimatedBytes = calculateEstimatedSize(fileList.totalDuration);
  sizeElement.textContent = `~ ${formatFileSize(estimatedBytes)}`;
}

/**
 * Calculates estimated output file size in bytes
 */
function calculateEstimatedSize(totalDurationSeconds: number): number {
  if (!totalDurationSeconds || totalDurationSeconds <= 0) {
    return 0;
  }

  // Base calculation: duration * bitrate / 8 (convert bits to bytes)
  let sizeBytes =
    (totalDurationSeconds * currentState.encoderSettings.bitrateKbps * 1000) /
    8;

  // Adjust for stereo (roughly 1.5x mono at same bitrate)
  if (currentState.encoderSettings.channels === "stereo") {
    sizeBytes *= 1.5;
  }

  // Add M4B container overhead (approximately 3%)
  sizeBytes *= 1.03;

  return Math.round(sizeBytes);
}

/**
 * Shows an error message in the output panel
 */
function showOutputError(message: string): void {
  console.error("Output Panel Error:", message);
}

/**
 * Gets current output configuration for processing
 */
export function getCurrentOutputConfig(): OutputConfig {
  if (!currentState.outputDirectory) {
    throw new Error("Output directory not selected");
  }

  // Return base directory; backend handles structure generation
  let encoderSettings = currentState.encoderSettings;
  const provider = (window as WindowWithEncoderProvider)
    .EncoderSettingsProvider;
  if (provider) {
    const raw = provider();
    if (raw) {
      encoderSettings = toBoundaryEncoderSettings(raw);
    }
  }

  return {
    encoderSettings,
    sampleRate: currentState.sampleRate,
    outputPath: currentState.outputDirectory,
    useSubdirPattern: currentState.useSubdirPattern,
    filenamePattern: currentState.filenamePattern,
  };
}

/**
 * Updates output panel when file list changes
 */
export function onFileListChange(): void {
  updateEstimatedSize();
}

/**
 * Updates output panel when metadata changes
 */
export function onMetadataChange(): void {
  updateOutputPath();
  updateEstimatedSize();
}
