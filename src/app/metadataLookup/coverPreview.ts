import {
	createCoverArtPreviewScheduler,
	DEFAULT_COVER_ART_PREVIEW_CACHE_ENTRIES,
	type CoverArtPreviewState,
} from '../../lib/media/coverArtPreviewScheduler';

export const MAX_METADATA_LOOKUP_PREVIEW_CACHE_ENTRIES = DEFAULT_COVER_ART_PREVIEW_CACHE_ENTRIES;

export type MetadataLookupCoverPreviewState = CoverArtPreviewState;

export type MetadataLookupCoverPreviews = {
	clear(): void;
	cancel(): void;
	getState(coverUrl: string | null | undefined): MetadataLookupCoverPreviewState;
	schedule(coverUrls: ReadonlyArray<string | null | undefined>): void;
	loadBytes(coverUrl: string): Promise<number[]>;
	fetch(coverUrl: string): Promise<void>;
};

export function createMetadataLookupCoverPreviews(deps: {
	readonly loadCoverArtFromUrl: (url: string) => Promise<number[]>;
	readonly onChange: () => void;
}): MetadataLookupCoverPreviews {
	const previewByUrl: Record<string, MetadataLookupCoverPreviewState> = {};
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
		failureLogMessage: 'Failed to load metadata lookup cover preview:',
	});

	return {
		clear: () => scheduler.clear(),
		cancel: () => scheduler.cancel(),
		getState: (coverUrl) => scheduler.getState(coverUrl),
		schedule: (coverUrls) => scheduler.schedule(coverUrls, deps.loadCoverArtFromUrl),
		loadBytes: (coverUrl) => scheduler.loadBytes(coverUrl, deps.loadCoverArtFromUrl),
		fetch: (coverUrl) => scheduler.fetch(coverUrl, deps.loadCoverArtFromUrl),
	};
}
