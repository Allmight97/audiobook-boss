export {
	initStatusPanel,
	isStatusPanelProcessing,
	pushStatusPanelTransientStatus,
	triggerCancelAllFromStatusPanel,
	triggerProcessFromStatusPanel,
} from './controller';
export { default as StatusTransportIsland } from './StatusTransportIsland.svelte';
import { STATUS_PANEL_DEFAULT_STEP_COLOR, statusPanelViewState } from './viewState.svelte';

/** True when retained foreground preview state owns the transport line. */
export function readStatusTransportActive(): boolean {
	return (
		statusPanelViewState.isProcessing ||
		statusPanelViewState.stepColor !== STATUS_PANEL_DEFAULT_STEP_COLOR
	);
}
