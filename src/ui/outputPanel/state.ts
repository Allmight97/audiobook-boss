/**
 * Output panel state management
 */
import type {
  EncoderSettings,
  EncoderChannelConfig,
  SampleRateConfig,
  OutputNamingConfig,
} from "../../types/audio";
import { defaultEncoderSettings } from "../../types/audio";

/**
 * State interface for output panel configuration
 */
export interface OutputPanelState {
  encoderSettings: EncoderSettings;
  sampleRate: SampleRateConfig;
  outputDirectory: string;
  absCompatible: boolean;
  absIncludeYear: boolean;
}

/**
 * Current output panel state (module-level singleton)
 */
let currentState: OutputPanelState = {
  encoderSettings: { ...defaultEncoderSettings() },
  sampleRate: { explicit: 22050 },
  outputDirectory: "",
  absCompatible: true,
  absIncludeYear: false,
};

/**
 * Gets the current output panel state
 */
export function getState(): OutputPanelState {
  return currentState;
}

/**
 * Builds output naming configuration from current state
 */
export function getOutputNamingConfig(): OutputNamingConfig {
  return {
    absCompatible: currentState.absCompatible,
    includeYear: currentState.absIncludeYear,
  };
}

/**
 * Updates encoder settings in state
 */
export function updateEncoderSettings(
  updates: Partial<EncoderSettings>
): void {
  currentState.encoderSettings = {
    ...currentState.encoderSettings,
    ...updates,
  };
}

/**
 * Updates sample rate in state
 */
export function updateSampleRate(value: string): void {
  currentState.sampleRate =
    value === "auto" ? "auto" : { explicit: parseInt(value) };
}

/**
 * Updates channels in state
 */
export function updateChannels(value: string): void {
  const channels: EncoderChannelConfig =
    value === "mono" ? "mono" : value === "stereo" ? "stereo" : "auto";
  currentState.encoderSettings = { ...currentState.encoderSettings, channels };
}

/**
 * Updates output directory in state
 */
export function updateOutputDirectory(path: string): void {
  currentState.outputDirectory = path;
}

/**
 * Updates ABS-compatible toggle in state
 */
export function updateAbsCompatible(enabled: boolean): void {
  currentState.absCompatible = enabled;
}

/**
 * Updates ABS year toggle in state
 */
export function updateAbsIncludeYear(enabled: boolean): void {
  currentState.absIncludeYear = enabled;
}

/**
 * Loads initial state from DOM elements
 */
export function loadInitialState(): void {
  const bitrateSelect = document.getElementById(
    "output-bitrate"
  ) as HTMLSelectElement;
  const sampleRateSelect = document.getElementById(
    "output-samplerate"
  ) as HTMLSelectElement;
  const channelsSelect = document.getElementById(
    "output-channels"
  ) as HTMLSelectElement;
  const absStructureCheckbox = document.getElementById(
    "output-abs-structure"
  ) as HTMLInputElement;
  const absYearCheckbox = document.getElementById(
    "output-abs-include-year"
  ) as HTMLInputElement;

  if (bitrateSelect) {
    currentState.encoderSettings = {
      ...currentState.encoderSettings,
      bitrateKbps: parseInt(
        bitrateSelect.value
      ) as EncoderSettings["bitrateKbps"],
    };
  }

  if (sampleRateSelect) {
    updateSampleRate(sampleRateSelect.value);
  }

  if (channelsSelect) {
    updateChannels(channelsSelect.value);
  }

  if (absStructureCheckbox) {
    currentState.absCompatible = absStructureCheckbox.checked;
  }

  if (absYearCheckbox) {
    currentState.absIncludeYear = absYearCheckbox.checked;
  }
}
