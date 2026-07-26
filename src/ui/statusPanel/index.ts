export {
	clearStatusPanelRetainedFeedback,
	initStatusPanel,
	isStatusPanelProcessing,
	pushStatusPanelTransientStatus,
	triggerCancelAllFromStatusPanel,
	triggerProcessFromStatusPanel,
} from './controller';
export { default as StatusTransportIsland } from './StatusTransportIsland.svelte';
import { statusPanelViewState } from './viewState.svelte';

/**
 * True while a retained foreground preview is actually processing. Feedback
 * (error/success colors) deliberately does NOT count: persistent feedback must
 * not outrank a live background operation in the transport union — it
 * resurfaces on the idle branch instead (StatusTransportIsland renders it).
 */
export function readStatusTransportProcessing(): boolean {
	return statusPanelViewState.isProcessing;
}
