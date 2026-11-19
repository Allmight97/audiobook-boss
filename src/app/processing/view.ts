import type { ChildCounts, ChildJobStatus, WorkProgressStage } from '../../lib/generated/tauri';

export type JobListItem = {
	readonly childJobId: string;
	readonly label: string;
	readonly status: ChildJobStatus['kind'];
	readonly statusText: string;
	readonly stage: WorkProgressStage;
	readonly percentage: number;
	readonly canCancel: boolean;
};

export type StepTone = 'normal' | 'error';

export type StatusView = {
	readonly coverArtSrc: string | null;
	readonly jobItems: ReadonlyArray<JobListItem>;
	readonly counts: ChildCounts;
	readonly progressPercentage: number;
	readonly statusText: string;
	readonly stepText: string;
	readonly stepTone: StepTone;
	readonly concurrencyText: string;
	readonly progressActive: boolean;
};

export const IDLE_STATUS_VIEW: StatusView = {
	coverArtSrc: null,
	jobItems: [],
	counts: { queued: 0, running: 0, completed: 0, skipped: 0, cancelled: 0, failed: 0 },
	progressPercentage: 0,
	statusText: 'Idle',
	stepText: 'Current Step: Ready',
	stepTone: 'normal',
	concurrencyText: '',
	progressActive: false,
};
