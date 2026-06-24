import {
	createCoverArtPreviewScheduler,
	type CoverArtPreviewState,
} from '../../lib/media/coverArtPreviewScheduler.svelte';

export type RemoteSourceCoverPreviewState = CoverArtPreviewState;

const remoteSourceCoverPreviewByUrl = $state<Record<string, RemoteSourceCoverPreviewState>>({});

const remoteSourceCoverPreviewScheduler = createCoverArtPreviewScheduler(
	remoteSourceCoverPreviewByUrl,
	{
		failureLogMessage: 'Failed to load remote source cover preview:',
	},
);

export function clearRemoteSourceCoverPreviewCache(): void {
	remoteSourceCoverPreviewScheduler.clear();
}

export function cancelRemoteSourceCoverPreviewSchedule(): void {
	remoteSourceCoverPreviewScheduler.cancel();
}

export function getRemoteSourceCoverPreviewState(
	coverUrl: string | null | undefined,
): RemoteSourceCoverPreviewState {
	return remoteSourceCoverPreviewScheduler.getState(coverUrl);
}

export function scheduleRemoteSourceCoverPreviews(
	coverUrls: ReadonlyArray<string | null | undefined>,
	loadCoverArtFromUrl: (url: string) => Promise<number[]>,
): void {
	remoteSourceCoverPreviewScheduler.schedule(coverUrls, loadCoverArtFromUrl);
}
