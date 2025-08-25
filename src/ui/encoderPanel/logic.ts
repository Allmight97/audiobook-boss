import { ENABLE_FDK, ENABLE_VBR } from './featureFlags';
import { queryDom } from './dom';

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
};


