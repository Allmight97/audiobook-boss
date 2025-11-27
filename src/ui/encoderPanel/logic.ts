import { ENABLE_FDK, ENABLE_VBR } from './featureFlags';
import { queryDom } from './dom';
import type { EncoderSettingsLike, EncoderFlavor, AacProfile, EncoderSettingsV2 } from '../../types/encoder';
import { VALID_ENCODER_BITRATES } from '../../types/audio';

type WindowWithEncoderProvider = Window & {
  EncoderSettingsProvider?: () => EncoderSettingsLike;
};

/**
 * Reads the current encoder settings from the DOM
 */
const getEncoderSettingsFromDom = (): EncoderSettingsLike => {
  const dom = queryDom();
  if (!dom.root) return undefined;

  // Read encoder flavor from adv-encoder select
  const encoderValue = dom.encoderSelect?.value ?? 'aac_at';
  const flavorMap: Record<string, EncoderFlavor> = {
    'native_aac': 'native_aac',
    'aac_at': 'aac_at',
    'external_fdk': 'external_fdk',
  };
  const flavor: EncoderFlavor = flavorMap[encoderValue] ?? 'aac_at';

  // Read profile from adv-profile select
  const profileValue = dom.profileSelect?.value ?? 'he';
  const profileMap: Record<string, AacProfile> = {
    'lc': 'lc',
    'he': 'he',
    'he_v2': 'he_v2',
  };
  const profile: AacProfile = profileMap[profileValue] ?? 'he';

  // Read channels from output-channels select
  const channelsValue = dom.channelsSelect?.value ?? 'mono';
  const channels: 1 | 2 = channelsValue === 'stereo' ? 2 : 1;

  // Read bitrate from output-bitrate select
  const bitrateSelect = document.getElementById('output-bitrate') as HTMLSelectElement | null;
  const bitrateValue = parseInt(bitrateSelect?.value ?? '64', 10);
  const bitrateKbps = ([...VALID_ENCODER_BITRATES].includes(bitrateValue as typeof VALID_ENCODER_BITRATES[number])
    ? bitrateValue
    : 64) as EncoderSettingsV2['bitrateKbps'];

  return {
    flavor,
    profile,
    channels,
    bitrateKbps,
    vbr: { enabled: false }, // VBR disabled for now
    fdkAfterburner: false, // FDK disabled for now
  };
};

export const initializeEncoderPanelLogic = (): void => {
  const dom = queryDom();
  if (!dom.root) return; // Panel not present in this view; no-op

  // Enforce disabled placeholders per phase decisions
  if (!ENABLE_VBR) {
    if (dom.aacAtVbrEnabled) dom.aacAtVbrEnabled.disabled = true;
    if (dom.aacAtVbrQuality) dom.aacAtVbrQuality.disabled = true;
    if (dom.fdkVbrEnabled) dom.fdkVbrEnabled.disabled = true;
    if (dom.fdkVbrLevel) dom.fdkVbrLevel.disabled = true;
  }

  if (!ENABLE_FDK) {
    if (dom.fdkAfterburner) dom.fdkAfterburner.disabled = true;
    if (dom.fdkStatus) dom.fdkStatus.textContent = 'FDK: disabled (future)';
  }

  // Expose EncoderSettingsProvider to window for statusPanel consumption
  (window as WindowWithEncoderProvider).EncoderSettingsProvider = getEncoderSettingsFromDom;
};


