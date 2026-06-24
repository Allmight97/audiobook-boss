import {
	createCoverArtPreviewScheduler,
	DEFAULT_COVER_ART_PREVIEW_CACHE_ENTRIES,
	DEFAULT_COVER_ART_PREVIEW_CONCURRENCY,
	type CoverArtPreviewState,
} from '../../lib/media/coverArtPreviewScheduler.svelte';

export const MAX_METADATA_LOOKUP_PREVIEW_CONCURRENCY = DEFAULT_COVER_ART_PREVIEW_CONCURRENCY;
export const MAX_METADATA_LOOKUP_PREVIEW_CACHE_ENTRIES = DEFAULT_COVER_ART_PREVIEW_CACHE_ENTRIES;

export type MetadataLookupCoverPreviewState = CoverArtPreviewState;

const metadataLookupCoverPreviewByUrl = $state<Record<string, MetadataLookupCoverPreviewState>>({});

const metadataLookupCoverPreviewScheduler = createCoverArtPreviewScheduler(
	metadataLookupCoverPreviewByUrl,
	{
		failureLogMessage: 'Failed to load metadata lookup cover preview:',
	},
);

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

export function getCachedMetadataLookupCoverBytes(coverUrl: string): number[] | null {
	return metadataLookupCoverPreviewScheduler.getCachedBytes(coverUrl);
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
