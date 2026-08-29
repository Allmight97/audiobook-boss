import type { AtomRegistry } from '../runtime/reactivity';
import { Atom } from '../runtime/reactivity';
import type { JobListItem } from './viewTypes';

export type StatusView = {
	readonly coverArtDataUrl: string | null;
	readonly jobItems: ReadonlyArray<JobListItem>;
	readonly progressPercentage: number;
	readonly statusText: string;
	readonly statusTextLockUntilEpochMs: number;
	readonly stepText: string;
	readonly stepColor: string;
	readonly concurrencyText: string;
	readonly isProcessing: boolean;
	readonly cancelAllPending: boolean;
};

const DEFAULT_STATUS_VIEW: StatusView = {
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

export const statusViewAtom = Atom.make<StatusView>(DEFAULT_STATUS_VIEW).pipe(Atom.keepAlive);

let snapshot: StatusView = DEFAULT_STATUS_VIEW;
let boundRegistry: AtomRegistry.AtomRegistry | null = null;
let statusMessageLockTimeoutId: number | null = null;
let statusBeforeUserMessageLock: string | null = null;
let queuedStatusAfterUserMessageLock: string | null = null;

export function bindStatusViewRegistry(registry: AtomRegistry.AtomRegistry | null): void {
	boundRegistry = registry;
	if (registry) {
		registry.set(statusViewAtom, snapshot);
	}
}

function commit(): void {
	boundRegistry?.set(statusViewAtom, snapshot);
}

function patch(next: Partial<StatusView>): void {
	snapshot = { ...snapshot, ...next };
	commit();
}

export function getStatusView(): StatusView {
	return snapshot;
}

function normalizeStatusLockTtlMs(ttlMs: number): number {
	if (!Number.isFinite(ttlMs)) return 0;
	return Math.max(0, Math.trunc(ttlMs));
}

export function isStatusPanelUserMessageLockActive(nowMs: number = Date.now()): boolean {
	return snapshot.statusTextLockUntilEpochMs > nowMs;
}

export function clearStatusPanelUserMessageLock(): void {
	if (statusMessageLockTimeoutId !== null) {
		window.clearTimeout(statusMessageLockTimeoutId);
		statusMessageLockTimeoutId = null;
	}

	const restoredStatus = queuedStatusAfterUserMessageLock ?? statusBeforeUserMessageLock;
	snapshot = {
		...snapshot,
		statusTextLockUntilEpochMs: 0,
		statusText: restoredStatus ?? snapshot.statusText,
	};
	statusBeforeUserMessageLock = null;
	queuedStatusAfterUserMessageLock = null;
	commit();
}

export function pushStatusPanelUserMessageLock(
	value: string,
	ttlMs: number = DEFAULT_USER_STATUS_LOCK_TTL_MS,
): void {
	if (!isStatusPanelUserMessageLockActive()) {
		statusBeforeUserMessageLock = snapshot.statusText;
		queuedStatusAfterUserMessageLock = null;
	}

	if (statusMessageLockTimeoutId !== null) {
		window.clearTimeout(statusMessageLockTimeoutId);
		statusMessageLockTimeoutId = null;
	}

	const lockTtlMs = normalizeStatusLockTtlMs(ttlMs);
	snapshot = {
		...snapshot,
		statusText: value,
		statusTextLockUntilEpochMs: lockTtlMs > 0 ? Date.now() + lockTtlMs : 0,
	};
	commit();

	if (lockTtlMs > 0) {
		statusMessageLockTimeoutId = window.setTimeout(() => {
			clearStatusPanelUserMessageLock();
		}, lockTtlMs);
	}
}

export function setStatusPanelCoverArtDataUrl(dataUrl: string | null): void {
	patch({ coverArtDataUrl: dataUrl });
}

export function setStatusPanelJobItems(items: JobListItem[]): void {
	patch({ jobItems: items });
}

export function setStatusPanelProgressPercentage(value: number): void {
	patch({ progressPercentage: value });
}

export function setStatusPanelStatusText(value: string): void {
	if (isStatusPanelUserMessageLockActive()) {
		queuedStatusAfterUserMessageLock = value;
		return;
	}
	if (snapshot.statusTextLockUntilEpochMs > 0) {
		clearStatusPanelUserMessageLock();
	}
	queuedStatusAfterUserMessageLock = null;
	statusBeforeUserMessageLock = null;
	patch({ statusText: value });
}

export function setStatusPanelStepText(value: string): void {
	patch({ stepText: value });
}

export function setStatusPanelStepColor(value: string): void {
	patch({ stepColor: value });
}

export function setStatusPanelConcurrencyText(value: string): void {
	patch({ concurrencyText: value });
}

export function setStatusPanelIsProcessing(isProcessing: boolean): void {
	patch({ isProcessing });
}

export function setStatusPanelCancelAllPending(isPending: boolean): void {
	patch({ cancelAllPending: isPending });
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
	setStatusPanelStepColor(DEFAULT_STATUS_VIEW.stepColor);
}

export function pushTransientStatusMessage(message: string, ttlMs?: number): void {
	pushStatusPanelUserMessageLock(message, ttlMs);
}

export function clearTransientStatusMessageLock(): void {
	clearStatusPanelUserMessageLock();
}

export function resetStatusPanelViewState(): void {
	snapshot = { ...DEFAULT_STATUS_VIEW, jobItems: [] };
	if (statusMessageLockTimeoutId !== null) {
		window.clearTimeout(statusMessageLockTimeoutId);
		statusMessageLockTimeoutId = null;
	}
	statusBeforeUserMessageLock = null;
	queuedStatusAfterUserMessageLock = null;
	commit();
}
