import type { ChildJobSnapshot, WorkProgressStage } from '../../types/workRuntime';

export type JobListItem = {
	readonly childJobId: string;
	readonly label: string;
	readonly status: ChildJobSnapshot['status'];
	readonly statusText: string;
	readonly stage: WorkProgressStage;
	readonly percentage: number;
	readonly canCancel: boolean;
};

export type StatusView = {
	readonly coverArtSrc: string | null;
	readonly jobItems: ReadonlyArray<JobListItem>;
	readonly progressPercentage: number;
	readonly statusText: string;
	readonly stepText: string;
	readonly stepColor: string;
	readonly concurrencyText: string;
	/** A preview is running or cancelling now; a queued one shows still progress. */
	readonly progressActive: boolean;
};

export const DEFAULT_STATUS_VIEW: StatusView = {
	coverArtSrc: null,
	jobItems: [],
	progressPercentage: 0,
	statusText: 'Idle',
	stepText: 'Current Step: Waiting for files...',
	stepColor: 'var(--text-primary)',
	concurrencyText: '',
	progressActive: false,
};

type StatusPublisher = (view: StatusView) => void;

export type StatusViewStore = {
	bindPublisher: (next: StatusPublisher | null) => void;
	reset: () => void;
	setCoverArtSrc: (src: string | null) => void;
	setJobItems: (items: JobListItem[]) => void;
	setProgressPercentage: (value: number) => void;
	setStatusText: (value: string) => void;
	setStepText: (value: string) => void;
	setStepColor: (value: string) => void;
	setConcurrencyText: (value: string) => void;
	setProgressActive: (active: boolean) => void;
	showError: (message: string) => void;
	showInfo: (message: string) => void;
};

export function createStatusViewStore(): StatusViewStore {
	let snapshot: StatusView = DEFAULT_STATUS_VIEW;
	let publisher: StatusPublisher | null = null;

	function commit(): void {
		publisher?.(snapshot);
	}

	function patch(next: Partial<StatusView>): void {
		snapshot = { ...snapshot, ...next };
		commit();
	}

	const store: StatusViewStore = {
		bindPublisher(next) {
			publisher = next;
			next?.(snapshot);
		},
		reset() {
			snapshot = { ...DEFAULT_STATUS_VIEW, jobItems: [] };
			commit();
		},
		setCoverArtSrc(src) {
			patch({ coverArtSrc: src });
		},
		setJobItems(items) {
			patch({ jobItems: items });
		},
		setProgressPercentage(value) {
			patch({ progressPercentage: value });
		},
		setStatusText(value) {
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
		setProgressActive(progressActive) {
			patch({ progressActive });
		},
		showError(message) {
			store.setStepText(`Error: ${message}`);
			store.setStepColor('var(--text-error, #ef4444)');
		},
		showInfo(message) {
			store.setStepText(message);
			store.setStepColor(DEFAULT_STATUS_VIEW.stepColor);
		},
	};

	return store;
}
