import {
	createCoverArtPreviewScheduler,
	DEFAULT_COVER_ART_PREVIEW_CACHE_ENTRIES,
	type CoverArtPreviewState,
} from '../../lib/media/coverArtPreviewScheduler';

export const MAX_REMOTE_SOURCE_PREVIEW_CACHE_ENTRIES = DEFAULT_COVER_ART_PREVIEW_CACHE_ENTRIES;

export type RemoteSourceCoverPreviewState = CoverArtPreviewState;

export type RemoteSourceCoverPreviews = {
	clear(): void;
	cancel(): void;
	getState(coverUrl: string | null | undefined): RemoteSourceCoverPreviewState;
	schedule(coverUrls: ReadonlyArray<string | null | undefined>): void;
};

export function createRemoteSourceCoverPreviews(deps: {
	readonly loadCoverArtFromUrl: (url: string) => Promise<number[]>;
	readonly onChange: () => void;
}): RemoteSourceCoverPreviews {
	const previewByUrl: Record<string, RemoteSourceCoverPreviewState> = {};
	const reactivePreviewByUrl = new Proxy(previewByUrl, {
		set(target, key, value) {
			Reflect.set(target, key, value);
			deps.onChange();
			return true;
		},
		deleteProperty(target, key) {
			Reflect.deleteProperty(target, key);
			deps.onChange();
			return true;
		},
	});
	const scheduler = createCoverArtPreviewScheduler(reactivePreviewByUrl, {
		failureLogMessage: 'Failed to load remote source cover preview:',
	});

	return {
		clear: () => scheduler.clear(),
		cancel: () => scheduler.cancel(),
		getState: (coverUrl) => scheduler.getState(coverUrl),
		schedule: (coverUrls) => scheduler.schedule(coverUrls, deps.loadCoverArtFromUrl),
	};
}
