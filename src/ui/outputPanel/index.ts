/**
 * Output panel module - manages output directory, naming patterns, and encoder settings
 *
 * Re-exports public API from submodules.
 */
import type { OutputConfig } from "../../types/audio";
import {
  toBoundaryEncoderSettings,
  EncoderSettingsLike,
} from "../../types/encoder";
import { getOutputNamingConfig, getState, loadInitialState } from "./state";
import { setupEventHandlers } from "./handlers";
import {
  updateOutputPath,
  updateEstimatedSize,
  updateNamingOptionState,
} from "./dom";

type WindowWithEncoderProvider = Window & {
  EncoderSettingsProvider?: () => EncoderSettingsLike;
};

/**
 * Initializes the output panel with event handlers
 */
export function initOutputPanel(): void {
  setupEventHandlers();
  loadInitialState();
  updateNamingOptionState();
  updateOutputPath();
  updateEstimatedSize();
}

/**
 * Gets current output configuration for processing
 */
export function getCurrentOutputConfig(): OutputConfig {
  const state = getState();
  if (!state.outputDirectory) {
    throw new Error("Output directory not selected");
  }

  // Return base directory; backend handles structure generation
  let encoderSettings = state.encoderSettings;
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
    sampleRate: state.sampleRate,
    outputPath: state.outputDirectory,
    outputNaming: getOutputNamingConfig(),
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

// Re-export commonly needed items
export { getState } from "./state";
export { updateOutputPath, updateEstimatedSize, getCurrentMetadata } from "./dom";
export { sanitizeFilename, calculateOutputPath } from "./pathBuilder";
