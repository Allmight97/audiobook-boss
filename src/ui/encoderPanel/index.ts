import { initializeEncoderPanelLogic } from './logic';

export interface EncoderPanelOptions {
	onSettingsChange?: () => void;
}

export const initEncoderPanel = (_opts?: EncoderPanelOptions): void => {
	initializeEncoderPanelLogic();
};
