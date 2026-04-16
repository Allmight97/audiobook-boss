import {
	StatusPanel,
	clearStatusPanelTransientStatusLock,
	getStatusPanel,
	isStatusPanelProcessing,
	initStatusPanel as initStatusPanelLogic,
	pushStatusPanelTransientStatus,
} from './logic';

export function initStatusPanel(): StatusPanel {
	return initStatusPanelLogic();
}

export {
	StatusPanel,
	clearStatusPanelTransientStatusLock,
	isStatusPanelProcessing,
	getStatusPanel,
	pushStatusPanelTransientStatus,
};
export { default as StatusPanelIsland } from './StatusPanelIsland.svelte';
