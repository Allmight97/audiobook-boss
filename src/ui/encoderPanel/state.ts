import type { EncoderSettingsV2 } from '../../types/encoder';

const LS_KEY = 'abb.encoderPanel.v2';

export const loadState = (): Partial<EncoderSettingsV2> => {
  try {
    const raw = localStorage.getItem(LS_KEY);
    return raw ? JSON.parse(raw) : {};
  } catch {
    return {};
  }
};

export const saveState = (state: Partial<EncoderSettingsV2>): void => {
  try {
    localStorage.setItem(LS_KEY, JSON.stringify(state));
  } catch {}
};


