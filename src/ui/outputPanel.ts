// import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type { AudioSettings, ChannelConfig } from "../types/audio";
import type { AudiobookMetadata } from "../types/metadata";
import { currentFileList } from "./fileList";
import { formatFileSize } from "../types/audio";

interface OutputPanelState {
  bitrate: number;
  sampleRate: number;
  channels: ChannelConfig;
  outputDirectory: string;
  useSubdirPattern: boolean;
  filenamePattern: 'title_year' | 'author_title';
}

let currentState: OutputPanelState = {
  bitrate: 64,
  sampleRate: 22050,
  channels: 'Mono',
  outputDirectory: '',
  useSubdirPattern: true,
  filenamePattern: 'title_year'
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
}

/**
 * Sets up audio settings event handlers
 */
function setupSettingsHandlers(): void {
  const bitrateSelect = document.getElementById('output-bitrate') as HTMLSelectElement;
  const sampleRateSelect = document.getElementById('output-samplerate') as HTMLSelectElement;
  const channelsSelect = document.getElementById('output-channels') as HTMLSelectElement;

  if (bitrateSelect) {
    bitrateSelect.addEventListener('change', handleBitrateChange);
  }

  if (sampleRateSelect) {
    sampleRateSelect.addEventListener('change', handleSampleRateChange);
  }

  if (channelsSelect) {
    channelsSelect.addEventListener('change', handleChannelsChange);
  }
}

/**
 * Sets up directory selection event handlers
 */
function setupDirectoryHandlers(): void {
  const browseButton = document.getElementById('output-dir-browse') as HTMLButtonElement;
  const subdirCheckbox = document.getElementById('output-subdir-pattern') as HTMLInputElement;

  if (browseButton) {
    browseButton.addEventListener('click', handleDirectoryBrowse);
  }

  if (subdirCheckbox) {
    subdirCheckbox.addEventListener('change', handleSubdirPatternChange);
  }
}

/**
 * Sets up filename pattern event handlers
 */
function setupPatternHandlers(): void {
  const patternRadios = document.querySelectorAll('input[name="filename_pattern"]');
  patternRadios.forEach(radio => {
    radio.addEventListener('change', handleFilenamePatternChange);
  });
}

/**
 * Handles bitrate selection change
 */
function handleBitrateChange(event: Event): void {
  const target = event.target as HTMLSelectElement;
  currentState.bitrate = parseInt(target.value);
  updateEstimatedSize();
}

/**
 * Handles sample rate selection change  
 */
function handleSampleRateChange(event: Event): void {
  const target = event.target as HTMLSelectElement;
  const value = target.value;
  currentState.sampleRate = value === 'auto' ? 22050 : parseInt(value);
  updateEstimatedSize();
}

/**
 * Handles channel configuration change
 */
function handleChannelsChange(event: Event): void {
  const target = event.target as HTMLSelectElement;
  currentState.channels = target.value === 'mono' ? 'Mono' : 'Stereo';
  updateEstimatedSize();
}

/**
 * Handles directory browse button click
 */
async function handleDirectoryBrowse(): Promise<void> {
  try {
    const selectedPath = await open({
      directory: true,
      multiple: false,
      title: 'Select Output Directory'
    });

    if (selectedPath && typeof selectedPath === 'string') {
      currentState.outputDirectory = selectedPath;
      updateOutputPath();
    }
  } catch (error) {
    console.error('Error selecting directory:', error);
    showOutputError('Failed to select directory');
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
  currentState.filenamePattern = target.value as 'title_year' | 'author_title';
  updateOutputPath();
}

/**
 * Loads initial state from HTML elements
 */
function loadInitialState(): void {
  const bitrateSelect = document.getElementById('output-bitrate') as HTMLSelectElement;
  const sampleRateSelect = document.getElementById('output-samplerate') as HTMLSelectElement;
  const channelsSelect = document.getElementById('output-channels') as HTMLSelectElement;

  if (bitrateSelect) {
    currentState.bitrate = parseInt(bitrateSelect.value);
  }

  if (sampleRateSelect) {
    const value = sampleRateSelect.value;
    currentState.sampleRate = value === 'auto' ? 22050 : parseInt(value);
  }

  if (channelsSelect) {
    currentState.channels = channelsSelect.value === 'mono' ? 'Mono' : 'Stereo';
  }
}

/**
 * Updates the output path display
 */
function updateOutputPath(): void {
  const outputPathInput = document.getElementById('output-dir-text') as HTMLInputElement;
  if (!outputPathInput) return;

  if (!currentState.outputDirectory) {
    outputPathInput.value = 'Please select output directory...';
    return;
  }

  const metadata = getCurrentMetadata();
  const calculatedPath = calculateOutputPath(metadata);
  outputPathInput.value = calculatedPath;
}

/**
 * Calculates the full output path based on current settings
 */
function calculateOutputPath(metadata: AudiobookMetadata): string {
  let basePath = currentState.outputDirectory;

  if (currentState.useSubdirPattern) {
    basePath = buildSubdirectoryPath(basePath, metadata);
  }

  const filename = buildFilename(metadata);
  return `${basePath}/${filename}`;
}

/**
 * Builds subdirectory path using metadata pattern
 */
function buildSubdirectoryPath(basePath: string, metadata: AudiobookMetadata): string {
  const author = metadata.author || 'Unknown Author';
  const series = metadata.series || '';
  const year = metadata.year || new Date().getFullYear();
  const title = metadata.title || 'Untitled';

  let subdirPath = `${basePath}/${author}`;

  if (series) {
    subdirPath += `/${series}`;
  }

  subdirPath += `/${year}-${title}`;
  return subdirPath;
}

/**
 * Builds output filename based on pattern selection
 */
function buildFilename(metadata: AudiobookMetadata): string {
  const title = metadata.title || 'Untitled';
  const author = metadata.author || 'Unknown Author';
  const year = metadata.year || new Date().getFullYear();

  if (currentState.filenamePattern === 'author_title') {
    return `${author} - ${title}.m4b`;
  }

  return `${title} (${year}).m4b`;
}

/**
 * Gets current metadata from the metadata panel
 */
function getCurrentMetadata(): AudiobookMetadata {
  // For now, read from DOM elements until metadata panel is implemented
  const getElementValue = (id: string): string => {
    const element = document.getElementById(id) as HTMLInputElement;
    return element?.value || '';
  };

  return {
    title: getElementValue('meta-title'),
    author: getElementValue('meta-author'),
    album: getElementValue('meta-album'),
    narrator: getElementValue('meta-narrator'),
    year: parseInt(getElementValue('meta-year')) || undefined,
    genre: getElementValue('meta-genre'),
    description: getElementValue('meta-description'),
    series: getElementValue('meta-series')
  };
}

/**
 * Updates the estimated output size display
 */
function updateEstimatedSize(): void {
  const sizeElement = document.getElementById('output-estimated-size');
  if (!sizeElement) return;

  const fileList = currentFileList;
  if (!fileList || !fileList.files.length) {
    sizeElement.textContent = '~ --- MB';
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
  let sizeBytes = (totalDurationSeconds * currentState.bitrate * 1000) / 8;

  // Adjust for stereo (roughly 1.5x mono at same bitrate)
  if (currentState.channels === 'Stereo') {
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
  console.error('Output Panel Error:', message);
  // Could add visual error display here in the future
}

/**
 * Gets current audio settings for processing
 */
export function getCurrentAudioSettings(): AudioSettings {
  if (!currentState.outputDirectory) {
    throw new Error('Output directory not selected');
  }

  const metadata = getCurrentMetadata();
  const outputPath = calculateOutputPath(metadata);

  return {
    bitrate: currentState.bitrate,
    channels: currentState.channels,
    sampleRate: currentState.sampleRate,
    outputPath: outputPath
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
