import { coverArtBytesToDataUrl } from '../coverArt';

export type MetadataLookupCoverPreviewState =
	| { status: 'idle' }
	| { status: 'loading' }
	| { status: 'ready'; bytes: number[]; dataUrl: string }
	| { status: 'error' };

export const metadataLookupCoverPreviewByIndex = $state<Record<number, MetadataLookupCoverPreviewState>>(
	{},
);

const inflightByIndex = new Map<number, Promise<void>>();

export function clearMetadataLookupCoverPreviewCache(): void {
	for (const key of Object.keys(metadataLookupCoverPreviewByIndex)) {
		delete metadataLookupCoverPreviewByIndex[Number(key)];
	}
	inflightByIndex.clear();
}

export function getMetadataLookupCoverPreviewState(
	index: number,
): MetadataLookupCoverPreviewState {
	return metadataLookupCoverPreviewByIndex[index] ?? { status: 'idle' };
}

export function getCachedMetadataLookupCoverBytes(index: number): number[] | null {
	const state = metadataLookupCoverPreviewByIndex[index];
	return state?.status === 'ready' ? state.bytes : null;
}

export async function fetchMetadataLookupCoverPreview(
	index: number,
	coverUrl: string,
	loadCoverArtFromUrl: (url: string) => Promise<number[]>,
): Promise<void> {
	const existing = metadataLookupCoverPreviewByIndex[index];
	if (existing?.status === 'ready') {
		return;
	}
	const inflight = inflightByIndex.get(index);
	if (inflight) {
		return inflight;
	}

	metadataLookupCoverPreviewByIndex[index] = { status: 'loading' };
	const promise = loadCoverArtFromUrl(coverUrl)
		.then((bytes) => {
			metadataLookupCoverPreviewByIndex[index] = {
				status: 'ready',
				bytes,
				dataUrl: coverArtBytesToDataUrl(bytes),
			};
		})
		.catch(() => {
			metadataLookupCoverPreviewByIndex[index] = { status: 'error' };
		})
		.finally(() => {
			inflightByIndex.delete(index);
		});

	inflightByIndex.set(index, promise);
	return promise;
}

export function prefetchMetadataLookupCoverPreviews(
	results: ReadonlyArray<{ coverUrl: string | null | undefined }>,
	loadCoverArtFromUrl: (url: string) => Promise<number[]>,
): void {
	for (const [index, result] of results.entries()) {
		if (!result.coverUrl) {
			continue;
		}
		void fetchMetadataLookupCoverPreview(index, result.coverUrl, loadCoverArtFromUrl);
	}
}
