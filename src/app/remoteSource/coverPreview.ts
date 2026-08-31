import {
	createCoverArtPreviewScheduler,
	DEFAULT_COVER_ART_PREVIEW_CACHE_ENTRIES,
	type CoverArtPreviewState,
} from '../../lib/media/coverArtPreviewScheduler';

export const MAX_REMOTE_SOURCE_PREVIEW_CACHE_ENTRIES = DEFAULT_COVER_ART_PREVIEW_CACHE_ENTRIES;

export type RemoteSourceCoverPreviewState = CoverArtPreviewState;

const remoteSourceCoverPreviewByUrl: Record<string, RemoteSourceCoverPreviewState> = {};
const previewListeners = new Set<() => void>();

function notifyPreviewChanged(): void {
	for (const listener of previewListeners) {
		listener();
	}
}

export function subscribeRemoteSourceCoverPreviews(listener: () => void): () => void {
	previewListeners.add(listener);
	return () => {
		previewListeners.delete(listener);
	};
}

const reactivePreviewByUrl = new Proxy(remoteSourceCoverPreviewByUrl, {
	set(target, key, value) {
		Reflect.set(target, key, value);
		notifyPreviewChanged();
		return true;
	},
	deleteProperty(target, key) {
		Reflect.deleteProperty(target, key);
		notifyPreviewChanged();
		return true;
	},
});

const remoteSourceCoverPreviewScheduler = createCoverArtPreviewScheduler(reactivePreviewByUrl, {
	failureLogMessage: 'Failed to load remote source cover preview:',
});

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
	loadCoverPreview: (url: string) => Promise<string>,
): void {
	remoteSourceCoverPreviewScheduler.schedule(coverUrls, loadCoverPreview);
}
