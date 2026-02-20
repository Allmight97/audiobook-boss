/**
 * DOM manipulation and UI update helpers for StatusPanel
 *
 * This module handles all direct DOM interactions and UI state updates.
 * Keeps DOM concerns separated from business logic.
 */
import {
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

/**
 * Cached DOM elements for the status panel
 */
interface StatusPanelElements {
	progressBar: HTMLElement;
	percentageElement: HTMLElement;
	statusText: HTMLElement;
	stepText: HTMLElement;
	processButton: HTMLButtonElement;
	artThumbnail: HTMLElement;
	cancelAllButton: HTMLButtonElement;
	jobList: HTMLElement;
	concurrencyStatus: HTMLElement;
}

let cachedElements: StatusPanelElements | null = null;

/**
 * Initialize and cache DOM elements
 * @returns StatusPanelElements or null if elements not found
 */
export function initializeElements(): StatusPanelElements | null {
	if (cachedElements) {
		return cachedElements;
	}

	const progressBar = document.getElementById('progress-bar') as HTMLElement;
	const percentageElement = document.getElementById('percentage-processed') as HTMLElement;
	const statusText = document.getElementById('status-text') as HTMLElement;
	const stepText = document.getElementById('step-text') as HTMLElement;
	const processButton = document.getElementById('process-button') as HTMLButtonElement;
	const cancelAllButton = document.getElementById('cancel-all-button') as HTMLButtonElement;
	const jobList = document.getElementById('job-list') as HTMLElement;
	const concurrencyStatus = document.getElementById('concurrency-status') as HTMLElement;
	const artThumbnail = document.querySelector('.art-thumbnail') as HTMLElement;

	if (
		!progressBar ||
		!percentageElement ||
		!statusText ||
		!stepText ||
		!processButton ||
		!cancelAllButton ||
		!jobList ||
		!concurrencyStatus ||
		!artThumbnail
	) {
		console.error('StatusPanel DOM: Required DOM elements not found');
		return null;
	}

	cachedElements = {
		progressBar,
		percentageElement,
		statusText,
		stepText,
		processButton,
		cancelAllButton,
		jobList,
		concurrencyStatus,
		artThumbnail,
	};

	return cachedElements;
}

/**
 * Update the progress bar width
 * @param percentage - Progress percentage (0-100)
 */
export function updateProgressBar(percentage: number): void {
	setStatusPanelProgressPercentage(percentage);

	const elements = cachedElements || initializeElements();
	if (!elements) return;

	elements.progressBar.style.width = `${percentage}%`;
}

/**
 * Update the percentage text display
 * @param percentage - Progress percentage (0-100)
 */
export function updatePercentageText(percentage: number): void {
	const elements = cachedElements || initializeElements();
	if (!elements) return;

	elements.percentageElement.textContent = `${percentage.toFixed(1)}%`;
}

/**
 * Update the main status text
 * @param statusText - Status text to display
 */
export function updateStatusText(statusText: string): void {
	const elements = cachedElements || initializeElements();
	if (!elements) return;

	const lockUntil = Number(elements.statusText.dataset.userStatusLockUntil ?? '0');
	if (Number.isFinite(lockUntil) && lockUntil > Date.now()) {
		return;
	}
	if (elements.statusText.dataset.userStatusLockUntil) {
		delete elements.statusText.dataset.userStatusLockUntil;
	}

	setStatusPanelStatusText(statusText);
	elements.statusText.textContent = statusText;
}

/**
 * Update the step text with optional color styling
 * @param message - Step message to display
 * @param color - Optional CSS color value
 */
export function updateStepText(message: string, color?: string): void {
	const elements = cachedElements || initializeElements();
	if (!elements) return;
	const resolvedColor = color ?? 'var(--text-primary)';

	setStatusPanelStepText(message);
	setStatusPanelStepColor(resolvedColor);
	elements.stepText.textContent = message;
	elements.stepText.style.color = resolvedColor;
}

/**
 * Update the concurrency status line
 * @param message - Concurrency message to display
 */
export function updateConcurrencyStatus(message: string): void {
	setStatusPanelConcurrencyText(message);

	const elements = cachedElements || initializeElements();
	if (!elements) return;

	elements.concurrencyStatus.textContent = message;
}

/**
 * Update the process button state and appearance
 * @param isProcessing - Whether processing is currently active
 */
export function updateProcessButton(_isProcessing: boolean): void {
	const elements = cachedElements || initializeElements();
	if (!elements) return;

	// Keep a consistent "Process" label even when multiple jobs are running
	// to encourage additional submissions while a cancel-all control handles stops.
	elements.processButton.textContent = 'Process Audiobook';
	elements.processButton.className = 'btn-pill btn-pill-primary';
}

/**
 * Display cover art in the art thumbnail area
 * @param dataUrl - Base64 data URL of the image
 */
export function displayCoverArt(dataUrl: string): void {
	setStatusPanelCoverArtDataUrl(dataUrl);
}

/**
 * Reset art thumbnail to placeholder state
 */
export function resetArtThumbnail(): void {
	setStatusPanelCoverArtDataUrl(null);
}

/**
 * Get the process button element for event listener attachment
 * @returns HTMLButtonElement or null if not found
 */
export function getProcessButton(): HTMLButtonElement | null {
	const elements = cachedElements || initializeElements();
	return elements?.processButton || null;
}

/**
 * Get the cancel-all button element for event listener attachment
 */
export function getCancelAllButton(): HTMLButtonElement | null {
	const elements = cachedElements || initializeElements();
	return elements?.cancelAllButton || null;
}

/**
 * Toggle cancel-all button pending state while cancellation request is in flight.
 */
export function setCancelAllButtonPending(isPending: boolean): void {
	setStatusPanelCancelAllPending(isPending);

	const cancelButton = getCancelAllButton();
	if (!cancelButton) return;

	cancelButton.disabled = isPending;
}

/**
 * Show error message in step text with error styling
 * @param message - Error message to display
 */
export function showError(message: string): void {
	updateStepText(`Error: ${message}`, 'var(--text-error, #ef4444)');
}

/**
 * Show success message in step text with success styling
 * @param message - Success message to display
 */
export function showSuccess(message: string): void {
	updateStepText(message, 'var(--text-success, #10b981)');
}

/**
 * Show info message in step text with default styling
 * @param message - Info message to display
 */
export function showInfo(message: string): void {
	updateStepText(message);
}

export function renderJobList(jobs: JobListItem[]): void {
	setStatusPanelJobItems(jobs);
}

export function resetStatusPanelDomCache(): void {
	cachedElements = null;
}
