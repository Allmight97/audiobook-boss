import type { ChildJobSnapshot, WorkProgressStage } from '../../lib/generated/tauri';

export type JobListItem = {
	readonly childJobId: string;
	readonly label: string;
	readonly status: ChildJobSnapshot['status'];
	readonly statusText: string;
	readonly stage: WorkProgressStage;
	readonly percentage: number;
	readonly canCancel: boolean;
};

export type StepTone = 'normal' | 'error';

export type StatusView = {
	readonly coverArtSrc: string | null;
	readonly jobItems: ReadonlyArray<JobListItem>;
	readonly progressPercentage: number;
	readonly statusText: string;
	readonly stepText: string;
	readonly stepTone: StepTone;
	readonly concurrencyText: string;
	/** A preview is running or cancelling now; a queued one shows still progress. */
	readonly progressActive: boolean;
};

export const IDLE_STATUS_VIEW: StatusView = {
	coverArtSrc: null,
	jobItems: [],
	progressPercentage: 0,
	statusText: 'Idle',
	stepText: 'Current Step: Ready',
	stepTone: 'normal',
	concurrencyText: '',
	progressActive: false,
};
