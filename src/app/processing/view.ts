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

export const DEFAULT_STATUS_VIEW: StatusView = {
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

type StatusPublisher = (view: StatusView) => void;

export type StatusViewStore = {
	snapshot(): StatusView;
	bindPublisher(next: StatusPublisher | null): void;
	reset(): void;
	setCoverArtDataUrl(dataUrl: string | null): void;
	setJobItems(items: JobListItem[]): void;
	setProgressPercentage(value: number): void;
	setStatusText(value: string): void;
	setStepText(value: string): void;
	setStepColor(value: string): void;
	setConcurrencyText(value: string): void;
	setIsProcessing(isProcessing: boolean): void;
	setCancelAllPending(isPending: boolean): void;
	showError(message: string): void;
	showSuccess(message: string): void;
	showInfo(message: string): void;
	pushTransient(message: string, ttlMs?: number): void;
};

function normalizeStatusLockTtlMs(ttlMs: number): number {
	if (!Number.isFinite(ttlMs)) return 0;
	return Math.max(0, Math.trunc(ttlMs));
}

export function createStatusViewStore(): StatusViewStore {
	let snapshot: StatusView = DEFAULT_STATUS_VIEW;
	let publisher: StatusPublisher | null = null;
	let statusMessageLockTimeoutId: number | null = null;
	let statusBeforeUserMessageLock: string | null = null;
	let queuedStatusAfterUserMessageLock: string | null = null;

	function commit(): void {
		publisher?.(snapshot);
	}

	function patch(next: Partial<StatusView>): void {
		snapshot = { ...snapshot, ...next };
		commit();
	}

	function isUserMessageLockActive(nowMs: number = Date.now()): boolean {
		return snapshot.statusTextLockUntilEpochMs > nowMs;
	}

	function clearUserMessageLock(): void {
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

	function pushUserMessageLock(
		value: string,
		ttlMs: number = DEFAULT_USER_STATUS_LOCK_TTL_MS,
	): void {
		if (!isUserMessageLockActive()) {
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
				clearUserMessageLock();
			}, lockTtlMs);
		}
	}

	const store: StatusViewStore = {
		snapshot: () => snapshot,
		bindPublisher(next) {
			publisher = next;
			next?.(snapshot);
		},
		reset() {
			snapshot = { ...DEFAULT_STATUS_VIEW, jobItems: [] };
			if (statusMessageLockTimeoutId !== null) {
				window.clearTimeout(statusMessageLockTimeoutId);
				statusMessageLockTimeoutId = null;
			}
			statusBeforeUserMessageLock = null;
			queuedStatusAfterUserMessageLock = null;
			commit();
		},
		setCoverArtDataUrl(dataUrl) {
			patch({ coverArtDataUrl: dataUrl });
		},
		setJobItems(items) {
			patch({ jobItems: items });
		},
		setProgressPercentage(value) {
			patch({ progressPercentage: value });
		},
		setStatusText(value) {
			if (isUserMessageLockActive()) {
				queuedStatusAfterUserMessageLock = value;
				return;
			}
			if (snapshot.statusTextLockUntilEpochMs > 0) {
				clearUserMessageLock();
			}
			queuedStatusAfterUserMessageLock = null;
			statusBeforeUserMessageLock = null;
			patch({ statusText: value });
		},
		setStepText(value) {
			patch({ stepText: value });
		},
		setStepColor(value) {
			patch({ stepColor: value });
		},
		setConcurrencyText(value) {
			patch({ concurrencyText: value });
		},
		setIsProcessing(isProcessing) {
			patch({ isProcessing });
		},
		setCancelAllPending(isPending) {
			patch({ cancelAllPending: isPending });
		},
		showError(message) {
			store.setStepText(`Error: ${message}`);
			store.setStepColor('var(--text-error, #ef4444)');
		},
		showSuccess(message) {
			store.setStepText(message);
			store.setStepColor('var(--text-success, #10b981)');
		},
		showInfo(message) {
			store.setStepText(message);
			store.setStepColor(DEFAULT_STATUS_VIEW.stepColor);
		},
		pushTransient(message, ttlMs) {
			pushUserMessageLock(message, ttlMs);
		},
	};

	return store;
}
