/**
 * StatusPanel module public API
 *
 * This module re-exports the public interface for StatusPanel,
 * maintaining the same import contract as the original single file.
 */

import {
	StatusPanel,
	clearStatusPanelTransientStatusLock,
	getStatusPanel,
	initStatusPanel as initStatusPanelLogic,
	pushStatusPanelTransientStatus,
} from './logic';

export function initStatusPanel(): StatusPanel {
	return initStatusPanelLogic();
}

export {
	StatusPanel,
	clearStatusPanelTransientStatusLock,
	getStatusPanel,
	pushStatusPanelTransientStatus,
};
export { default as StatusPanelIsland } from './StatusPanelIsland.svelte';
