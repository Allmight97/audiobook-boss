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

    if (!progressBar || !percentageElement || !statusText ||
        !stepText || !processButton || !cancelAllButton || !jobList || !concurrencyStatus || !artThumbnail) {
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
        artThumbnail
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
export function renderJobList(
    jobs: Array<{
        key: string;
        label: string;
        statusText: string;
        percentage?: number;
        canCancel: boolean;
        cancelId?: string;
        onCancel?: (id: string) => void;
    }>
): void {
    const elements = cachedElements || initializeElements();
    if (!elements) return;

    if (jobs.length === 0) {
        elements.jobList.textContent = '';
        return;
    }

    const fragment = document.createDocumentFragment();

    jobs.forEach((job) => {
        const row = document.createElement('div');
        row.className = 'flex items-center justify-between gap-2 mb-1';
        const label = document.createElement('span');
        const percentage =
            typeof job.percentage === 'number' ? ` (${job.percentage.toFixed(1)}%)` : '';
        label.textContent = `${job.label} • ${job.statusText}${percentage}`;
        label.className = 'flex-1';
        const cancelButton = document.createElement('button');
        cancelButton.textContent = 'Cancel';
        cancelButton.id = `cancel-${job.key}`;
        cancelButton.className = 'button-secondary';
        cancelButton.style.padding = '0.1rem 0.4rem';
        cancelButton.disabled = !job.canCancel;
        if (job.onCancel && job.canCancel && job.cancelId) {
            cancelButton.addEventListener('click', () => job.onCancel?.(job.cancelId as string));
        }
        row.appendChild(label);
        row.appendChild(cancelButton);
        fragment.appendChild(row);
    });

    elements.jobList.innerHTML = '';
    elements.jobList.appendChild(fragment);
}
