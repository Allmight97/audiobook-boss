import { coverArtBytesToDataUrl } from '../../lib/media/coverArtDataUrl';

export const MAX_METADATA_LOOKUP_PREVIEW_CONCURRENCY = 2;
export const MAX_METADATA_LOOKUP_PREVIEW_CACHE_ENTRIES = 64;

export type MetadataLookupCoverPreviewState =
	| { status: 'idle' }
	| { status: 'queued' }
	| { status: 'loading' }
	| { status: 'ready'; bytes: number[]; dataUrl: string }
	| { status: 'error' };

const metadataLookupCoverPreviewByUrl = $state<Record<string, MetadataLookupCoverPreviewState>>({});

const inflightByUrl = new Map<string, Promise<number[]>>();
const activePreviewTasks = new Set<number>();
const cacheOrder: string[] = [];

let nextPreviewTaskId = 0;
let previewScheduleGeneration = 0;
let queuedCoverUrls: string[] = [];
let visibleCoverUrls = new Set<string>();
let activeLoader: ((url: string) => Promise<number[]>) | null = null;

export function clearMetadataLookupCoverPreviewCache(): void {
	cancelMetadataLookupCoverPreviewSchedule();
	for (const key of Object.keys(metadataLookupCoverPreviewByUrl)) {
		delete metadataLookupCoverPreviewByUrl[key];
	}
	cacheOrder.length = 0;
	inflightByUrl.clear();
}

export function cancelMetadataLookupCoverPreviewSchedule(): void {
	previewScheduleGeneration += 1;
	queuedCoverUrls = [];
	visibleCoverUrls = new Set();
	activeLoader = null;
	for (const [coverUrl, state] of Object.entries(metadataLookupCoverPreviewByUrl)) {
		if (state.status === 'queued' || state.status === 'loading') {
			delete metadataLookupCoverPreviewByUrl[coverUrl];
		}
	}
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

export function scheduleMetadataLookupCoverPreviews(
	coverUrls: ReadonlyArray<string | null | undefined>,
	loadCoverArtFromUrl: (url: string) => Promise<number[]>,
): void {
	previewScheduleGeneration += 1;
	activeLoader = loadCoverArtFromUrl;
	const generation = previewScheduleGeneration;
	const uniqueUrls = uniqueCoverUrls(coverUrls);
	visibleCoverUrls = new Set(uniqueUrls);
	queuedCoverUrls = [];

	for (const coverUrl of Object.keys(metadataLookupCoverPreviewByUrl)) {
		if (!visibleCoverUrls.has(coverUrl)) {
			const state = metadataLookupCoverPreviewByUrl[coverUrl];
			if (state.status === 'queued' || state.status === 'loading') {
				delete metadataLookupCoverPreviewByUrl[coverUrl];
			}
		}
	}

	for (const coverUrl of uniqueUrls) {
		const state = metadataLookupCoverPreviewByUrl[coverUrl];
		if (state?.status === 'ready') {
			touchCacheEntry(coverUrl);
			continue;
		}
		const inflight = inflightByUrl.get(coverUrl);
		if (inflight) {
			metadataLookupCoverPreviewByUrl[coverUrl] = { status: 'loading' };
			attachScheduledInflightCompletion(coverUrl, inflight, generation);
			continue;
		}
		metadataLookupCoverPreviewByUrl[coverUrl] = { status: 'queued' };
		queuedCoverUrls.push(coverUrl);
	}

	pumpPreviewQueue(generation);
}

export async function loadMetadataLookupCoverBytes(
	coverUrl: string,
	loadCoverArtFromUrl: (url: string) => Promise<number[]>,
): Promise<number[]> {
	const existing = metadataLookupCoverPreviewByUrl[coverUrl];
	if (existing?.status === 'ready') {
		touchCacheEntry(coverUrl);
		return existing.bytes;
	}
	const inflight = inflightByUrl.get(coverUrl);
	if (inflight) {
		const generation = previewScheduleGeneration;
		return inflight.then((bytes) => {
			if (shouldCommitPreviewCompletion(coverUrl, generation, true)) {
				commitReadyPreview(coverUrl, bytes);
			}
			return bytes;
		});
	}
	return startPreviewFetch(coverUrl, loadCoverArtFromUrl, previewScheduleGeneration, true);
}

export async function fetchMetadataLookupCoverPreview(
	coverUrl: string,
	loadCoverArtFromUrl: (url: string) => Promise<number[]>,
): Promise<void> {
	await loadMetadataLookupCoverBytes(coverUrl, loadCoverArtFromUrl);
}

function uniqueCoverUrls(coverUrls: ReadonlyArray<string | null | undefined>): string[] {
	const unique = new Set<string>();
	for (const coverUrl of coverUrls) {
		if (coverUrl) {
			unique.add(coverUrl);
		}
	}
	return [...unique];
}

function attachScheduledInflightCompletion(
	coverUrl: string,
	inflight: Promise<number[]>,
	generation: number,
): void {
	void inflight
		.then((bytes) => {
			if (shouldCommitPreviewCompletion(coverUrl, generation, false)) {
				commitReadyPreview(coverUrl, bytes);
			}
		})
		.catch((error) => {
			if (shouldCommitPreviewCompletion(coverUrl, generation, false)) {
				console.warn('Failed to load metadata lookup cover preview:', error);
				metadataLookupCoverPreviewByUrl[coverUrl] = { status: 'error' };
				touchCacheEntry(coverUrl);
				prunePreviewCache();
			}
		});
}

function pumpPreviewQueue(generation: number): void {
	if (!activeLoader || generation !== previewScheduleGeneration) {
		return;
	}
	while (
		activePreviewTasks.size < MAX_METADATA_LOOKUP_PREVIEW_CONCURRENCY &&
		queuedCoverUrls.length > 0
	) {
		const coverUrl = queuedCoverUrls.shift();
		if (!coverUrl || !visibleCoverUrls.has(coverUrl)) {
			continue;
		}
		const state = metadataLookupCoverPreviewByUrl[coverUrl];
		if (state?.status === 'ready' || inflightByUrl.has(coverUrl)) {
			continue;
		}
		void startPreviewFetch(coverUrl, activeLoader, generation, false).catch(() => undefined);
	}
}

function startPreviewFetch(
	coverUrl: string,
	loadCoverArtFromUrl: (url: string) => Promise<number[]>,
	generation: number,
	allowOffscreenCompletion: boolean,
): Promise<number[]> {
	metadataLookupCoverPreviewByUrl[coverUrl] = { status: 'loading' };
	const taskId = ++nextPreviewTaskId;
	activePreviewTasks.add(taskId);
	let promise!: Promise<number[]>;
	promise = loadCoverArtFromUrl(coverUrl)
		.then((bytes): number[] => {
			if (shouldCommitPreviewCompletion(coverUrl, generation, allowOffscreenCompletion)) {
				commitReadyPreview(coverUrl, bytes);
			}
			return bytes;
		})
		.catch((error): never => {
			if (shouldCommitPreviewCompletion(coverUrl, generation, allowOffscreenCompletion)) {
				console.warn('Failed to load metadata lookup cover preview:', error);
				metadataLookupCoverPreviewByUrl[coverUrl] = { status: 'error' };
				touchCacheEntry(coverUrl);
				prunePreviewCache();
			}
			throw error;
		})
		.finally(() => {
			activePreviewTasks.delete(taskId);
			if (inflightByUrl.get(coverUrl) === promise) {
				inflightByUrl.delete(coverUrl);
			}
			pumpPreviewQueue(previewScheduleGeneration);
		});

	inflightByUrl.set(coverUrl, promise);
	return promise;
}

function commitReadyPreview(coverUrl: string, bytes: number[]): void {
	metadataLookupCoverPreviewByUrl[coverUrl] = {
		status: 'ready',
		bytes,
		dataUrl: coverArtBytesToDataUrl(bytes),
	};
	touchCacheEntry(coverUrl);
	prunePreviewCache();
}

function shouldCommitPreviewCompletion(
	coverUrl: string,
	generation: number,
	allowOffscreenCompletion: boolean,
): boolean {
	return (
		generation === previewScheduleGeneration &&
		(allowOffscreenCompletion || visibleCoverUrls.has(coverUrl))
	);
}

function touchCacheEntry(coverUrl: string): void {
	const existingIndex = cacheOrder.indexOf(coverUrl);
	if (existingIndex >= 0) {
		cacheOrder.splice(existingIndex, 1);
	}
	cacheOrder.push(coverUrl);
}

function prunePreviewCache(): void {
	let remainingCandidates = cacheOrder.length;
	while (
		cacheOrder.length > MAX_METADATA_LOOKUP_PREVIEW_CACHE_ENTRIES &&
		remainingCandidates > 0
	) {
		const coverUrl = cacheOrder.shift();
		remainingCandidates -= 1;
		if (!coverUrl) {
			continue;
		}
		if (inflightByUrl.has(coverUrl)) {
			cacheOrder.push(coverUrl);
			continue;
		}
		delete metadataLookupCoverPreviewByUrl[coverUrl];
	}
}
