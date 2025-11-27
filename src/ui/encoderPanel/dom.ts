export interface EncoderDomCache {
  root?: HTMLElement | null;
  encoderSelect?: HTMLSelectElement | null;
  bitrateModeSelect?: HTMLSelectElement | null;
  channelsSelect?: HTMLSelectElement | null;
  fdkVbrLevel?: HTMLInputElement | null;
  fdkVbrLabel?: HTMLElement | null;
  fdkAfterburner?: HTMLInputElement | null;
  fdkStatus?: HTMLElement | null;
  encoderAvailabilityHint?: HTMLElement | null;
  encoderNote?: HTMLElement | null;
  opusHint?: HTMLElement | null;
}

export const queryDom = (): EncoderDomCache => ({
  root: document.getElementById("advanced-settings-panel"),
  encoderSelect: document.getElementById(
    "adv-encoder"
  ) as HTMLSelectElement | null,
  bitrateModeSelect: document.getElementById(
    "adv-bitrate-mode"
  ) as HTMLSelectElement | null,
  channelsSelect: document.getElementById(
    "output-channels"
  ) as HTMLSelectElement | null,
  fdkVbrLevel: document.getElementById(
    "adv-fdk-vbr-level"
  ) as HTMLInputElement | null,
  fdkVbrLabel: document.getElementById("adv-fdk-vbr-label"),
  fdkAfterburner: document.getElementById(
    "adv-fdk-afterburner"
  ) as HTMLInputElement | null,
  fdkStatus: document.getElementById("adv-fdk-status"),
  encoderAvailabilityHint: document.getElementById("encoder-availability-hint"),
  encoderNote: document.getElementById("adv-encoder-note"),
  opusHint: document.getElementById("adv-opus-hint"),
});
