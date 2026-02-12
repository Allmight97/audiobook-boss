import { mount, unmount } from 'svelte';
import EncoderPanelIsland from './EncoderPanelIsland.svelte';
import { initializeEncoderPanelLogic } from './logic';

export interface EncoderPanelOptions {
	onSettingsChange?: () => void;
}

const ENCODER_PANEL_ROOT_ID = 'encoder-panel-root';

let mountedEncoderPanelRoot: HTMLElement | null = null;
let mountedEncoderPanelIsland: Parameters<typeof unmount>[0] | null = null;

const mountEncoderPanelIsland = (): void => {
	const panelRoot = document.getElementById(ENCODER_PANEL_ROOT_ID);
	if (!panelRoot) return;

	if (
		mountedEncoderPanelIsland &&
		mountedEncoderPanelRoot === panelRoot &&
		panelRoot.childElementCount > 0
	) {
		return;
	}

	if (mountedEncoderPanelIsland) {
		void unmount(mountedEncoderPanelIsland);
		mountedEncoderPanelIsland = null;
	}

	mountedEncoderPanelIsland = mount(EncoderPanelIsland, { target: panelRoot });
	mountedEncoderPanelRoot = panelRoot;
};

export const initEncoderPanel = (_opts?: EncoderPanelOptions): void => {
	mountEncoderPanelIsland();
	initializeEncoderPanelLogic();
};
