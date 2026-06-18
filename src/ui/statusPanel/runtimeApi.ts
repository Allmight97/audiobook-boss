import { setStatusPanelConcurrencyText } from './viewState.svelte';

export function updateStatusPanelConcurrencyStatus(message: string): void {
	setStatusPanelConcurrencyText(message);
}
