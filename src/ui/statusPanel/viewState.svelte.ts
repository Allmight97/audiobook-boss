import type { JobListItem } from './viewTypes';

type StatusPanelViewState = {
	coverArtDataUrl: string | null;
	jobItems: JobListItem[];
	progressPercentage: number;
	statusText: string;
	stepText: string;
	stepColor: string;
	concurrencyText: string;
	cancelAllPending: boolean;
};

export const statusPanelViewState = $state<StatusPanelViewState>({
	coverArtDataUrl: null,
	jobItems: [],
	progressPercentage: 0,
	statusText: 'Idle',
	stepText: 'Current Step: Waiting for files...',
	stepColor: 'var(--text-primary)',
	concurrencyText: '',
	cancelAllPending: false,
});

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
	statusPanelViewState.statusText = value;
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

export function setStatusPanelCancelAllPending(isPending: boolean): void {
	statusPanelViewState.cancelAllPending = isPending;
}
