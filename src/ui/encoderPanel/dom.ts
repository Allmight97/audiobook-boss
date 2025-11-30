export interface EncoderDomCache {
  root?: HTMLElement | null;
  encoderSelect?: HTMLSelectElement | null;
  bitrateModeSelect?: HTMLSelectElement | null;
  channelsSelect?: HTMLSelectElement | null;
  qualitySelect?: HTMLSelectElement | null;
  bitrateSelect?: HTMLSelectElement | null;
  qualityBitrateLabel?: HTMLElement | null;
  profileDisplay?: HTMLElement | null;
  estimatedBitrate?: HTMLElement | null;
  encoderOptionsSection?: HTMLElement | null;
  fdkOptions?: HTMLElement | null;
  nativeOptions?: HTMLElement | null;
  appleOptions?: HTMLElement | null;
  fdkAfterburner?: HTMLInputElement | null;
  nativeTwoloop?: HTMLInputElement | null;
  encoderAvailabilityHint?: HTMLElement | null;
}

export const queryDom = (): EncoderDomCache => ({
  root: document.getElementById("encoder-settings-panel"),
  encoderSelect: document.getElementById(
    "adv-encoder"
  ) as HTMLSelectElement | null,
  bitrateModeSelect: document.getElementById(
    "adv-bitrate-mode"
  ) as HTMLSelectElement | null,
  channelsSelect: document.getElementById(
    "output-channels"
  ) as HTMLSelectElement | null,
  qualitySelect: document.getElementById(
    "output-quality"
  ) as HTMLSelectElement | null,
  bitrateSelect: document.getElementById(
    "output-bitrate"
  ) as HTMLSelectElement | null,
  qualityBitrateLabel: document.getElementById("quality-bitrate-label"),
  profileDisplay: document.getElementById("encoder-profile-display"),
  estimatedBitrate: document.getElementById("estimated-bitrate"),
  encoderOptionsSection: document.getElementById("encoder-options-section"),
  fdkOptions: document.getElementById("fdk-options"),
  nativeOptions: document.getElementById("native-options"),
  appleOptions: document.getElementById("apple-options"),
  fdkAfterburner: document.getElementById(
    "adv-fdk-afterburner"
  ) as HTMLInputElement | null,
  nativeTwoloop: document.getElementById(
    "adv-native-twoloop"
  ) as HTMLInputElement | null,
  encoderAvailabilityHint: document.getElementById("encoder-availability-hint"),
});
