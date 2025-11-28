import { ENABLE_FDK } from "./featureFlags";
import { queryDom } from "./dom";
import { loadState, saveState } from "./state";
import type {
  EncoderSettingsLike,
  EncoderFlavor,
  EncoderSettingsV2,
} from "../../types/encoder";
import { VALID_ENCODER_BITRATES } from "../../types/audio";
import { bridge } from "../../lib/bridge";

type WindowWithEncoderProvider = Window & {
  EncoderSettingsProvider?: () => EncoderSettingsLike;
};

type EncoderAvailability = {
  fdk_available: boolean;
  aac_at_available: boolean;
  native_aac_available: boolean;
};

type VbrLevel = 1 | 2 | 3 | 4 | 5;

const clamp = (value: number, min: number, max: number): number =>
  Math.min(max, Math.max(min, value));

let cachedAvailability: EncoderAvailability | null = null;

/**
 * Reads the current encoder settings from the DOM
 */
const getEncoderSettingsFromDom = (): EncoderSettingsLike => {
  const dom = queryDom();
  if (!dom.root) return undefined;

  const encoderValue = dom.encoderSelect?.value as EncoderFlavor | undefined;
  const flavor: EncoderFlavor = encoderValue ?? "auto";

  const channelsValue = dom.channelsSelect?.value ?? "auto";
  const channels: EncoderSettingsV2["channels"] =
    channelsValue === "stereo" ? 2 : channelsValue === "mono" ? 1 : "auto";

  // Read bitrate from output-bitrate select
  const bitrateSelect = document.getElementById(
    "output-bitrate"
  ) as HTMLSelectElement | null;
  const bitrateValue = parseInt(bitrateSelect?.value ?? "64", 10);
  const bitrateKbps = (
    [...VALID_ENCODER_BITRATES].includes(
      bitrateValue as (typeof VALID_ENCODER_BITRATES)[number]
    )
    ? bitrateValue
      : 64
  ) as EncoderSettingsV2["bitrateKbps"];

  const bitrateModeValue = dom.bitrateModeSelect?.value ?? "vbr";
  const vbrLevel = clamp(
    Number(dom.fdkVbrLevel?.value ?? "3"),
    1,
    5
  ) as VbrLevel;
  const bitrateMode =
    bitrateModeValue === "cbr"
      ? { mode: "cbr" as const }
      : bitrateModeValue === "cvbr"
      ? { mode: "cvbr" as const }
      : { mode: "vbr" as const, level: vbrLevel };
  const vbr =
    flavor === "fdk_he_aac" ? { enabled: true, level: vbrLevel } : undefined;
  const fdkAfterburner =
    flavor === "fdk_he_aac" ? !!dom.fdkAfterburner?.checked : undefined;

  return {
    flavor,
    channels,
    bitrateKbps,
    bitrateMode,
    vbr,
    fdkAfterburner,
  };
};

export const initializeEncoderPanelLogic = (): void => {
  const dom = queryDom();
  if (!dom.root) return; // Panel not present in this view; no-op

  applyPersistedState();
  hydrateAvailability().finally(() => {
    syncAdvancedVisibility();
    persistState();
  });
  attachEventListeners();
  (window as WindowWithEncoderProvider).EncoderSettingsProvider =
    getEncoderSettingsFromDom;
};

const applyPersistedState = (): void => {
  const state = loadState();
  const dom = queryDom();
  if (state.flavor && dom.encoderSelect) {
    dom.encoderSelect.value = state.flavor;
  }
  if (state.channels && dom.channelsSelect) {
    dom.channelsSelect.value =
      state.channels === 2 ? "stereo" : state.channels === 1 ? "mono" : "auto";
  }
  if (state.bitrateKbps) {
    const bitrateSelect = document.getElementById(
      "output-bitrate"
    ) as HTMLSelectElement | null;
    if (bitrateSelect) {
      bitrateSelect.value = String(state.bitrateKbps);
    }
  }
  if (state.bitrateMode && dom.bitrateModeSelect) {
    dom.bitrateModeSelect.value = state.bitrateMode.mode;
  }
  if (state.vbr?.level && dom.fdkVbrLevel) {
    dom.fdkVbrLevel.value = String(state.vbr.level);
    updateFdkVbrLabel(state.vbr.level);
  }
  if (state.fdkAfterburner && dom.fdkAfterburner) {
    dom.fdkAfterburner.checked = state.fdkAfterburner;
  }
};

const attachEventListeners = (): void => {
  const dom = queryDom();
  dom.encoderSelect?.addEventListener("change", () => {
    syncAdvancedVisibility();
    persistState();
  });
  dom.bitrateModeSelect?.addEventListener("change", () => {
    syncAdvancedVisibility();
    persistState();
  });
  dom.channelsSelect?.addEventListener("change", persistState);
  document
    .getElementById("output-bitrate")
    ?.addEventListener("change", persistState);
  dom.fdkAfterburner?.addEventListener("change", persistState);
  dom.fdkVbrLevel?.addEventListener("input", () => {
    const level = clamp(Number(dom.fdkVbrLevel?.value ?? "3"), 1, 5);
    updateFdkVbrLabel(level);
    persistState();
  });
};

const updateFdkVbrLabel = (level: number): void => {
  const dom = queryDom();
  if (dom.fdkVbrLabel) {
    dom.fdkVbrLabel.textContent = `Quality ${level} (~${
      level >= 3 ? 60 : 48 + level * 4
    } kbps)`;
  }
};

const persistState = (): void => {
  const settings = getEncoderSettingsFromDom();
  if (settings) {
    saveState(settings);
  }
};

const hydrateAvailability = async (): Promise<void> => {
  try {
    cachedAvailability = await bridge.invoke<EncoderAvailability>(
      "list_available_encoders"
    );
  } catch (error) {
    console.warn("Failed to load encoder availability", error);
    cachedAvailability = null;
  }
  updateAvailabilityHint();
};

const updateAvailabilityHint = (): void => {
  const dom = queryDom();
  if (!dom.encoderAvailabilityHint) return;
  if (!cachedAvailability) {
    dom.encoderAvailabilityHint.textContent =
      "Encoder availability unknown (offline).";
    return;
  }
  const parts: string[] = [];
  parts.push(
    cachedAvailability.fdk_available
      ? "FDK detected."
      : "FDK missing — install FFmpeg with libfdk_aac."
  );
  parts.push(
    cachedAvailability.aac_at_available
      ? "Apple AAC available."
      : "Apple AAC not detected (install on macOS)."
  );
  dom.encoderAvailabilityHint.textContent = parts.join(" ");
};

const syncAdvancedVisibility = (): void => {
  const dom = queryDom();
  const flavor =
    (dom.encoderSelect?.value as EncoderFlavor | undefined) ?? "auto";
  const fdkControls = document.getElementById("adv-fdk-controls");
  if (fdkControls) {
    fdkControls.style.display = flavor === "fdk_he_aac" ? "grid" : "none";
  }
  if (dom.fdkAfterburner) {
    dom.fdkAfterburner.disabled = !ENABLE_FDK;
  }
  if (dom.encoderNote) {
    dom.encoderNote.textContent = getEncoderNote(flavor);
  }
  enforceBitrateModeCompatibility(flavor);
  disableDisallowedEncoders();
};

const getEncoderNote = (flavor: EncoderFlavor): string => {
  switch (flavor) {
    case "aac_at":
      return "Apple AAC uses CVBR and is ideal for Apple devices.";
    case "fdk_he_aac":
      return "FDK HE-AAC delivers ~60 kbps VBR with afterburner for speech.";
    case "native_aac":
      return "Native AAC uses CBR twoloop mode for compatibility.";
    case "auto":
    default:
      return "Auto selects FDK when available, otherwise Apple AAC or native AAC.";
  }
};

const enforceBitrateModeCompatibility = (flavor: EncoderFlavor): void => {
  const dom = queryDom();
  const select = dom.bitrateModeSelect;
  if (!select) return;
  const setValue = (value: string) => {
    select.value = value;
  };
  select
    .querySelectorAll("option")
    .forEach((option) => (option.disabled = false));
  if (flavor === "aac_at") {
    setValue("cvbr");
    select.querySelectorAll("option").forEach((option) => {
      if (option.value !== "cvbr") option.disabled = true;
    });
  } else if (flavor === "native_aac") {
    setValue("cbr");
    select.querySelectorAll("option").forEach((option) => {
      if (option.value !== "cbr") option.disabled = true;
    });
  } else {
    setValue("vbr");
    select.querySelectorAll("option").forEach((option) => {
      if (option.value !== "vbr") option.disabled = true;
    });
  }
};

const disableDisallowedEncoders = (): void => {
  const dom = queryDom();
  const select = dom.encoderSelect;
  if (!select) return;
  const availability = cachedAvailability;
  Array.from(select.options).forEach((option) => {
    switch (option.value) {
      case "fdk_he_aac":
        option.disabled = !ENABLE_FDK || !availability?.fdk_available;
        break;
      case "aac_at":
        option.disabled = availability ? !availability.aac_at_available : false;
        break;
      case "native_aac":
        option.disabled = availability
          ? !availability.native_aac_available
          : false;
        break;
      default:
        option.disabled = false;
    }
  });
  if (select.selectedOptions[0]?.disabled) {
    const fallback = Array.from(select.options).find((opt) => !opt.disabled);
    if (fallback) {
      select.value = fallback.value;
    }
  }
};
