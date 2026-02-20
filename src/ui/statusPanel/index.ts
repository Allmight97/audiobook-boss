/**
 * StatusPanel module public API
 *
 * This module re-exports the public interface for StatusPanel,
 * maintaining the same import contract as the original single file.
 */

import { mount, unmount } from 'svelte';
import StatusPanelIsland from './StatusPanelIsland.svelte';
import {
	StatusPanel,
	clearStatusPanelTransientStatusLock,
	getStatusPanel,
	initStatusPanel as initStatusPanelLogic,
	pushStatusPanelTransientStatus,
} from './logic';

const STATUS_PANEL_ROOT_ID = 'status-panel-root';

let mountedStatusPanelRoot: HTMLElement | null = null;
let mountedStatusPanelIsland: Parameters<typeof unmount>[0] | null = null;

function mountStatusPanelIsland(): void {
	const statusPanelRoot = document.getElementById(STATUS_PANEL_ROOT_ID);
	if (!statusPanelRoot) return;

	if (
		mountedStatusPanelIsland &&
		mountedStatusPanelRoot === statusPanelRoot &&
		statusPanelRoot.childElementCount > 0
	) {
		return;
	}

	if (mountedStatusPanelIsland) {
		void unmount(mountedStatusPanelIsland);
		mountedStatusPanelIsland = null;
	}

	mountedStatusPanelIsland = mount(StatusPanelIsland, { target: statusPanelRoot });
	mountedStatusPanelRoot = statusPanelRoot;
}

export function initStatusPanel(): StatusPanel {
	mountStatusPanelIsland();
	return initStatusPanelLogic();
}

export {
	StatusPanel,
	clearStatusPanelTransientStatusLock,
	getStatusPanel,
	pushStatusPanelTransientStatus,
};
