import type { EncoderSettingsV2 } from '../../types/encoder';

const LS_KEY = 'abb.encoderPanel.v2';

export const loadState = (): Partial<EncoderSettingsV2> => {
  try {
    const raw = localStorage.getItem(LS_KEY);
    return raw ? JSON.parse(raw) : {};
  } catch (error) {
    // FALLBACK[FB-013]: trigger=localStorage read/parse unavailable in current runtime
    // observe=console warn with persisted-state key and error details
    // sunset=2026-06-30 issue=#195
    console.warn(
      `FALLBACK[FB-013] unable to load encoder panel state for '${LS_KEY}'`,
      error
    );
    return {};
  }
};

export const saveState = (state: Partial<EncoderSettingsV2>): void => {
  try {
    localStorage.setItem(LS_KEY, JSON.stringify(state));
  } catch (error) {
    console.warn(
      `FALLBACK[FB-013] unable to persist encoder panel state for '${LS_KEY}'`,
      error
    );
  }
};

