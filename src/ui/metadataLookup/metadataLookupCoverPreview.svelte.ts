import { coverArtBytesToDataUrl } from '../coverArt';

export type MetadataLookupCoverPreviewState =
	| { status: 'idle' }
	| { status: 'loading' }
	| { status: 'ready'; bytes: number[]; dataUrl: string }
	| { status: 'error' };

const metadataLookupCoverPreviewByUrl = $state<Record<string, MetadataLookupCoverPreviewState>>({});

const inflightByUrl = new Map<string, Promise<void>>();

export function clearMetadataLookupCoverPreviewCache(): void {
	for (const key of Object.keys(metadataLookupCoverPreviewByUrl)) {
		delete metadataLookupCoverPreviewByUrl[key];
	}
	inflightByUrl.clear();
}

export function getMetadataLookupCoverPreviewState(
	coverUrl: string | null | undefined,
): MetadataLookupCoverPreviewState {
	if (!coverUrl) {
		return { status: 'idle' };
	}
	return metadataLookupCoverPreviewByUrl[coverUrl] ?? { status: 'idle' };
}

export function getCachedMetadataLookupCoverBytes(coverUrl: string): number[] | null {
	const state = metadataLookupCoverPreviewByUrl[coverUrl];
	return state?.status === 'ready' ? state.bytes : null;
}

export async function fetchMetadataLookupCoverPreview(
	coverUrl: string,
	loadCoverArtFromUrl: (url: string) => Promise<number[]>,
): Promise<void> {
	const existing = metadataLookupCoverPreviewByUrl[coverUrl];
	if (existing?.status === 'ready') {
		return;
	}
	const inflight = inflightByUrl.get(coverUrl);
	if (inflight) {
		return inflight;
	}

	metadataLookupCoverPreviewByUrl[coverUrl] = { status: 'loading' };
	const promise = loadCoverArtFromUrl(coverUrl)
		.then((bytes) => {
			metadataLookupCoverPreviewByUrl[coverUrl] = {
				status: 'ready',
				bytes,
				dataUrl: coverArtBytesToDataUrl(bytes),
			};
		})
		.catch((error) => {
			console.warn('Failed to load metadata lookup cover preview:', error);
			metadataLookupCoverPreviewByUrl[coverUrl] = { status: 'error' };
		})
		.finally(() => {
			inflightByUrl.delete(coverUrl);
		});

	inflightByUrl.set(coverUrl, promise);
	return promise;
}
