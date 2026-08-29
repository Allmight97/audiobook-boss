import {
	createCoverArtPreviewScheduler,
	DEFAULT_COVER_ART_PREVIEW_CACHE_ENTRIES,
	type CoverArtPreviewState,
} from '../../lib/media/coverArtPreviewScheduler';

export const MAX_METADATA_LOOKUP_PREVIEW_CACHE_ENTRIES = DEFAULT_COVER_ART_PREVIEW_CACHE_ENTRIES;

export type MetadataLookupCoverPreviewState = CoverArtPreviewState;

const metadataLookupCoverPreviewByUrl: Record<string, MetadataLookupCoverPreviewState> = {};
const previewListeners = new Set<() => void>();

function notifyPreviewChanged(): void {
	for (const listener of previewListeners) {
		listener();
	}
}

export function subscribeMetadataLookupCoverPreviews(listener: () => void): () => void {
	previewListeners.add(listener);
	return () => {
		previewListeners.delete(listener);
	};
}

const reactivePreviewByUrl = new Proxy(metadataLookupCoverPreviewByUrl, {
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

const metadataLookupCoverPreviewScheduler = createCoverArtPreviewScheduler(reactivePreviewByUrl, {
	failureLogMessage: 'Failed to load metadata lookup cover preview:',
});

export function clearMetadataLookupCoverPreviewCache(): void {
	metadataLookupCoverPreviewScheduler.clear();
}

export function cancelMetadataLookupCoverPreviewSchedule(): void {
	metadataLookupCoverPreviewScheduler.cancel();
}

export function getMetadataLookupCoverPreviewState(
	coverUrl: string | null | undefined,
): MetadataLookupCoverPreviewState {
	return metadataLookupCoverPreviewScheduler.getState(coverUrl);
}

export function scheduleMetadataLookupCoverPreviews(
	coverUrls: ReadonlyArray<string | null | undefined>,
	loadCoverArtFromUrl: (url: string) => Promise<number[]>,
): void {
	metadataLookupCoverPreviewScheduler.schedule(coverUrls, loadCoverArtFromUrl);
}

export async function loadMetadataLookupCoverBytes(
	coverUrl: string,
	loadCoverArtFromUrl: (url: string) => Promise<number[]>,
): Promise<number[]> {
	return metadataLookupCoverPreviewScheduler.loadBytes(coverUrl, loadCoverArtFromUrl);
}

export async function fetchMetadataLookupCoverPreview(
	coverUrl: string,
	loadCoverArtFromUrl: (url: string) => Promise<number[]>,
): Promise<void> {
	await metadataLookupCoverPreviewScheduler.fetch(coverUrl, loadCoverArtFromUrl);
}
