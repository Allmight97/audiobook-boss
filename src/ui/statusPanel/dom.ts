/**
 * DOM manipulation and UI update helpers for StatusPanel
 *
 * This module handles all direct DOM interactions and UI state updates.
 * Keeps DOM concerns separated from business logic.
 */

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

	elements.stepText.textContent = message;
	if (color) {
		elements.stepText.style.color = color;
	} else {
		elements.stepText.style.color = 'var(--text-primary)';
	}
}

/**
 * Update the concurrency status line
 * @param message - Concurrency message to display
 */
export function updateConcurrencyStatus(message: string): void {
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
	const elements = cachedElements || initializeElements();
	if (!elements) return;

	const coverArtImage = document.createElement('img');
	coverArtImage.src = dataUrl;
	coverArtImage.alt = 'Cover Art';
	coverArtImage.style.width = '100%';
	coverArtImage.style.height = '100%';
	coverArtImage.style.objectFit = 'cover';
	coverArtImage.style.borderRadius = '0.25rem';

	elements.artThumbnail.replaceChildren(coverArtImage);
}

/**
 * Reset art thumbnail to placeholder state
 */
export function resetArtThumbnail(): void {
	const elements = cachedElements || initializeElements();
	if (!elements) return;

	const placeholder = document.createElement('span');
	placeholder.textContent = 'Art';

	elements.artThumbnail.replaceChildren(placeholder);
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

/**
 * Render the active job list with optional per-job cancel affordances
 */
export type JobListItem = {
	key: string;
	label: string;
	statusText: string;
	percentage?: number;
	canCancel: boolean;
	cancelId?: string;
	onCancel?: (id: string) => void;
};

type JobSnapshot = {
	label: string;
	statusText: string;
	percentage?: number;
	isCancellable: boolean;
};

interface JobRowState {
	row: HTMLElement;
	label: HTMLElement;
	cancelButton: HTMLButtonElement;
	job: JobListItem;
	snapshot: JobSnapshot;
}

const jobListState = {
	rows: new Map<string, JobRowState>(),
	serialized: '',
};

function buildLabelText(job: JobListItem): string {
	const percentage = typeof job.percentage === 'number' ? ` (${job.percentage.toFixed(1)}%)` : '';
	return `${job.label} • ${job.statusText}${percentage}`;
}

function createSnapshot(job: JobListItem): JobSnapshot {
	return {
		label: job.label,
		statusText: job.statusText,
		percentage: job.percentage,
		isCancellable: job.canCancel && !!job.cancelId && !!job.onCancel,
	};
}

function areSnapshotsEqual(a: JobSnapshot, b: JobSnapshot): boolean {
	return (
		a.label === b.label &&
		a.statusText === b.statusText &&
		a.percentage === b.percentage &&
		a.isCancellable === b.isCancellable
	);
}

function updateCancelButtonState(button: HTMLButtonElement, isCancellable: boolean): void {
	const nextDisabled = !isCancellable;
	if (button.disabled !== nextDisabled) {
		button.disabled = nextDisabled;
	}
}

function createJobRow(job: JobListItem): JobRowState {
	const row = document.createElement('div');
	row.className = 'flex items-center justify-between gap-2 mb-1';
	const label = document.createElement('span');
	label.className = 'flex-1';
	const cancelButton = document.createElement('button');
	cancelButton.textContent = 'Cancel';
	cancelButton.id = `cancel-${job.key}`;
	cancelButton.className = 'button-secondary';
	cancelButton.style.padding = '0.1rem 0.4rem';

	const state: JobRowState = {
		row,
		label,
		cancelButton,
		job,
		snapshot: createSnapshot(job),
	};

	cancelButton.addEventListener('click', () => {
		const currentJob = state.job;
		if (currentJob.canCancel && currentJob.cancelId && currentJob.onCancel) {
			currentJob.onCancel(currentJob.cancelId);
		}
	});

	label.textContent = buildLabelText(job);
	updateCancelButtonState(cancelButton, state.snapshot.isCancellable);
	row.appendChild(label);
	row.appendChild(cancelButton);
	return state;
}

function updateJobRowState(state: JobRowState, job: JobListItem): void {
	state.job = job;
	const nextSnapshot = createSnapshot(job);
	if (!areSnapshotsEqual(state.snapshot, nextSnapshot)) {
		if (
			state.snapshot.label !== nextSnapshot.label ||
			state.snapshot.statusText !== nextSnapshot.statusText ||
			state.snapshot.percentage !== nextSnapshot.percentage
		) {
			state.label.textContent = buildLabelText(job);
		}
		if (state.snapshot.isCancellable !== nextSnapshot.isCancellable) {
			updateCancelButtonState(state.cancelButton, nextSnapshot.isCancellable);
		}
		state.snapshot = nextSnapshot;
	}
}

function serializeJobs(jobs: JobListItem[]): string {
	return JSON.stringify(
		jobs.map((job) => ({
			key: job.key,
			cancelId: job.cancelId ?? null,
			snapshot: createSnapshot(job),
		})),
	);
}

export function renderJobList(jobs: JobListItem[]): void {
	const elements = cachedElements || initializeElements();
	if (!elements) return;

	if (jobs.length === 0) {
		if (elements.jobList.childElementCount > 0) {
			elements.jobList.textContent = '';
		}
		jobListState.rows.clear();
		jobListState.serialized = '';
		return;
	}

	const serialized = serializeJobs(jobs);
	if (serialized === jobListState.serialized) {
		return;
	}
	jobListState.serialized = serialized;

	const seenKeys = new Set<string>();
	let expectedNode = elements.jobList.firstElementChild;
	jobs.forEach((job) => {
		seenKeys.add(job.key);
		let state = jobListState.rows.get(job.key);
		if (!state) {
			state = createJobRow(job);
			jobListState.rows.set(job.key, state);
		} else {
			updateJobRowState(state, job);
		}

		if (state.row !== expectedNode) {
			elements.jobList.insertBefore(state.row, expectedNode);
		}
		expectedNode = state.row.nextElementSibling;
	});

	const keysToRemove: string[] = [];
	for (const key of jobListState.rows.keys()) {
		if (!seenKeys.has(key)) {
			keysToRemove.push(key);
		}
	}
	keysToRemove.forEach((key) => {
		const stale = jobListState.rows.get(key);
		stale?.row.remove();
		jobListState.rows.delete(key);
	});
}

export function resetStatusPanelDomCache(): void {
	cachedElements = null;
	jobListState.rows.clear();
	jobListState.serialized = '';
}
