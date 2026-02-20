import type { JobListItem } from './viewTypes';

type StatusPanelViewState = {
	coverArtDataUrl: string | null;
	jobItems: JobListItem[];
};

export const statusPanelViewState = $state<StatusPanelViewState>({
	coverArtDataUrl: null,
	jobItems: [],
});

export function setStatusPanelCoverArtDataUrl(dataUrl: string | null): void {
	statusPanelViewState.coverArtDataUrl = dataUrl;
}

export function setStatusPanelJobItems(items: JobListItem[]): void {
	statusPanelViewState.jobItems = items;
}
