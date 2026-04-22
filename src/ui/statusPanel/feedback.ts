import {
	clearStatusPanelUserMessageLock,
	pushStatusPanelUserMessageLock,
	setStatusPanelCancelAllPending,
	setStatusPanelConcurrencyText,
	setStatusPanelCoverArtDataUrl,
	setStatusPanelJobItems,
	setStatusPanelProgressPercentage,
	setStatusPanelStatusText,
	setStatusPanelStepColor,
	setStatusPanelStepText,
} from './viewState.svelte';
import type { JobListItem } from './viewTypes';

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

export function updateProcessButton(_isProcessing: boolean): void {
	// Process button label/style are static in the island.
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

export function showError(message: string): void {
	updateStepText(`Error: ${message}`, 'var(--text-error, #ef4444)');
}

export function showSuccess(message: string): void {
	updateStepText(message, 'var(--text-success, #10b981)');
}

export function showInfo(message: string): void {
	updateStepText(message);
}

export function renderJobList(jobs: JobListItem[]): void {
	setStatusPanelJobItems(jobs);
}

export function pushTransientStatusMessage(message: string, ttlMs?: number): void {
	pushStatusPanelUserMessageLock(message, ttlMs);
}

export function clearTransientStatusMessageLock(): void {
	clearStatusPanelUserMessageLock();
}
