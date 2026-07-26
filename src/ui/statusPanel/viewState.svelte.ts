import type { JobListItem } from './viewTypes';

type StatusPanelViewState = {
	coverArtDataUrl: string | null;
	jobItems: JobListItem[];
	progressPercentage: number;
	statusText: string;
	statusTextLockUntilEpochMs: number;
	stepText: string;
	stepColor: string;
	concurrencyText: string;
	isProcessing: boolean;
	cancelAllPending: boolean;
};

const DEFAULT_STATUS_PANEL_VIEW_STATE: StatusPanelViewState = {
	coverArtDataUrl: null,
	jobItems: [],
	progressPercentage: 0,
	statusText: 'Idle',
	statusTextLockUntilEpochMs: 0,
	stepText: 'Current Step: Waiting for files...',
	stepColor: 'var(--text-primary)',
	concurrencyText: '',
	isProcessing: false,
	cancelAllPending: false,
};

const DEFAULT_USER_STATUS_LOCK_TTL_MS = 1_500;
let statusMessageLockTimeoutId: number | null = null;
let statusBeforeUserMessageLock: string | null = null;
let queuedStatusAfterUserMessageLock: string | null = null;

export const statusPanelViewState = $state<StatusPanelViewState>({
	...DEFAULT_STATUS_PANEL_VIEW_STATE,
});

function normalizeStatusLockTtlMs(ttlMs: number): number {
	if (!Number.isFinite(ttlMs)) return 0;
	return Math.max(0, Math.trunc(ttlMs));
}

export function isStatusPanelUserMessageLockActive(nowMs: number = Date.now()): boolean {
	return statusPanelViewState.statusTextLockUntilEpochMs > nowMs;
}

export function clearStatusPanelUserMessageLock(): void {
	if (statusMessageLockTimeoutId !== null) {
		window.clearTimeout(statusMessageLockTimeoutId);
		statusMessageLockTimeoutId = null;
	}

	statusPanelViewState.statusTextLockUntilEpochMs = 0;
	const restoredStatus = queuedStatusAfterUserMessageLock ?? statusBeforeUserMessageLock;
	if (restoredStatus !== null) {
		statusPanelViewState.statusText = restoredStatus;
	}

	statusBeforeUserMessageLock = null;
	queuedStatusAfterUserMessageLock = null;
}

export function pushStatusPanelUserMessageLock(
	value: string,
	ttlMs: number = DEFAULT_USER_STATUS_LOCK_TTL_MS,
): void {
	if (!isStatusPanelUserMessageLockActive()) {
		statusBeforeUserMessageLock = statusPanelViewState.statusText;
		queuedStatusAfterUserMessageLock = null;
	}

	if (statusMessageLockTimeoutId !== null) {
		window.clearTimeout(statusMessageLockTimeoutId);
		statusMessageLockTimeoutId = null;
	}

	const lockTtlMs = normalizeStatusLockTtlMs(ttlMs);
	statusPanelViewState.statusText = value;
	statusPanelViewState.statusTextLockUntilEpochMs = lockTtlMs > 0 ? Date.now() + lockTtlMs : 0;

	if (lockTtlMs > 0) {
		statusMessageLockTimeoutId = window.setTimeout(() => {
			clearStatusPanelUserMessageLock();
		}, lockTtlMs);
	}
}

export function setStatusPanelCoverArtDataUrl(dataUrl: string | null): void {
	statusPanelViewState.coverArtDataUrl = dataUrl;
}

export function setStatusPanelJobItems(items: JobListItem[]): void {
	statusPanelViewState.jobItems = items;
}

export function setStatusPanelProgressPercentage(value: number): void {
	statusPanelViewState.progressPercentage = value;
}

export function setStatusPanelStatusText(value: string): void {
	if (isStatusPanelUserMessageLockActive()) {
		queuedStatusAfterUserMessageLock = value;
		return;
	}
	if (statusPanelViewState.statusTextLockUntilEpochMs > 0) {
		clearStatusPanelUserMessageLock();
	}
	statusPanelViewState.statusText = value;
	queuedStatusAfterUserMessageLock = null;
	statusBeforeUserMessageLock = null;
}

export function setStatusPanelStepText(value: string): void {
	statusPanelViewState.stepText = value;
}

export function setStatusPanelStepColor(value: string): void {
	statusPanelViewState.stepColor = value;
}

export function setStatusPanelConcurrencyText(value: string): void {
	statusPanelViewState.concurrencyText = value;
}

export function setStatusPanelIsProcessing(isProcessing: boolean): void {
	statusPanelViewState.isProcessing = isProcessing;
}

export function setStatusPanelCancelAllPending(isPending: boolean): void {
	statusPanelViewState.cancelAllPending = isPending;
}

export function showError(message: string): void {
	setStatusPanelStepText(`Error: ${message}`);
	setStatusPanelStepColor('var(--text-error, #ef4444)');
}

export function showSuccess(message: string): void {
	setStatusPanelStepText(message);
	setStatusPanelStepColor('var(--text-success, #10b981)');
}

export function showInfo(message: string): void {
	setStatusPanelStepText(message);
	setStatusPanelStepColor(DEFAULT_STATUS_PANEL_VIEW_STATE.stepColor);
}

export function pushTransientStatusMessage(message: string, ttlMs?: number): void {
	pushStatusPanelUserMessageLock(message, ttlMs);
}

export function clearTransientStatusMessageLock(): void {
	clearStatusPanelUserMessageLock();
}

export function resetStatusPanelViewState(): void {
	statusPanelViewState.coverArtDataUrl = DEFAULT_STATUS_PANEL_VIEW_STATE.coverArtDataUrl;
	statusPanelViewState.jobItems = [...DEFAULT_STATUS_PANEL_VIEW_STATE.jobItems];
	statusPanelViewState.progressPercentage = DEFAULT_STATUS_PANEL_VIEW_STATE.progressPercentage;
	statusPanelViewState.statusText = DEFAULT_STATUS_PANEL_VIEW_STATE.statusText;
	statusPanelViewState.statusTextLockUntilEpochMs =
		DEFAULT_STATUS_PANEL_VIEW_STATE.statusTextLockUntilEpochMs;
	statusPanelViewState.stepText = DEFAULT_STATUS_PANEL_VIEW_STATE.stepText;
	statusPanelViewState.stepColor = DEFAULT_STATUS_PANEL_VIEW_STATE.stepColor;
	statusPanelViewState.concurrencyText = DEFAULT_STATUS_PANEL_VIEW_STATE.concurrencyText;
	statusPanelViewState.isProcessing = DEFAULT_STATUS_PANEL_VIEW_STATE.isProcessing;
	statusPanelViewState.cancelAllPending = DEFAULT_STATUS_PANEL_VIEW_STATE.cancelAllPending;

	if (statusMessageLockTimeoutId !== null) {
		window.clearTimeout(statusMessageLockTimeoutId);
		statusMessageLockTimeoutId = null;
	}
	statusBeforeUserMessageLock = null;
	queuedStatusAfterUserMessageLock = null;
}
