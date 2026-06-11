import {
	setStatusPanelCancelAllPending,
	setStatusPanelConcurrencyText,
	setStatusPanelCoverArtDataUrl,
	setStatusPanelIsProcessing,
	setStatusPanelJobItems,
	setStatusPanelProgressPercentage,
	setStatusPanelStatusText,
	setStatusPanelStepColor,
	setStatusPanelStepText,
} from './viewState.svelte';
import type { JobListItem } from './viewTypes';

export {
	clearTransientStatusMessageLock,
	pushTransientStatusMessage,
	showError,
	showInfo,
	showSuccess,
} from './viewState.svelte';

export function updateProgressBar(percentage: number): void {
	setStatusPanelProgressPercentage(percentage);
}

export function updatePercentageText(percentage: number): void {
	setStatusPanelProgressPercentage(percentage);
}

export function updateStatusText(statusText: string): void {
	setStatusPanelStatusText(statusText);
}

export function updateStepText(message: string, color?: string): void {
	setStatusPanelStepText(message);
	setStatusPanelStepColor(color ?? 'var(--text-primary)');
}

export function updateConcurrencyStatus(message: string): void {
	setStatusPanelConcurrencyText(message);
}

export function updateProcessButton(isProcessing: boolean): void {
	setStatusPanelIsProcessing(isProcessing);
}

export function displayCoverArt(dataUrl: string): void {
	setStatusPanelCoverArtDataUrl(dataUrl);
}

export function resetArtThumbnail(): void {
	setStatusPanelCoverArtDataUrl(null);
}

export function setCancelAllButtonPending(isPending: boolean): void {
	setStatusPanelCancelAllPending(isPending);
}

export function renderJobList(jobs: JobListItem[]): void {
	setStatusPanelJobItems(jobs);
}
