export interface EncoderDomCache {
  root?: HTMLElement | null;
  encoderSelect?: HTMLSelectElement | null;
  profileSelect?: HTMLSelectElement | null;
  channelsSelect?: HTMLSelectElement | null;
  aacAtVbrEnabled?: HTMLInputElement | null;
  aacAtVbrQuality?: HTMLInputElement | null;
  fdkVbrEnabled?: HTMLInputElement | null;
  fdkVbrLevel?: HTMLSelectElement | null;
  fdkAfterburner?: HTMLInputElement | null;
  fdkStatus?: HTMLElement | null;
  nativeOptimizeLC?: HTMLInputElement | null;
}

export const queryDom = (): EncoderDomCache => ({
  root: document.getElementById('advanced-settings-panel'),
  encoderSelect: document.getElementById('adv-encoder') as HTMLSelectElement | null,
  profileSelect: document.getElementById('adv-profile') as HTMLSelectElement | null,
  channelsSelect: document.getElementById('output-channels') as HTMLSelectElement | null,
  aacAtVbrEnabled: document.getElementById('adv-aacat-vbr-enabled') as HTMLInputElement | null,
  aacAtVbrQuality: document.getElementById('adv-aacat-vbr-quality') as HTMLInputElement | null,
  fdkVbrEnabled: document.getElementById('adv-fdk-vbr-enabled') as HTMLInputElement | null,
  fdkVbrLevel: document.getElementById('adv-fdk-vbr-level') as HTMLSelectElement | null,
  fdkAfterburner: document.getElementById('adv-fdk-afterburner') as HTMLInputElement | null,
  fdkStatus: document.getElementById('adv-fdk-status'),
  nativeOptimizeLC: document.getElementById('adv-native-optimize-lc') as HTMLInputElement | null,
});


