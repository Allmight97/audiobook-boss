import { coverArtBytesToDataUrl } from './coverArtDataUrl';
import { createBoundedGenerationQueue } from './boundedGenerationQueue';

export const DEFAULT_COVER_ART_PREVIEW_CONCURRENCY = 2;
export const DEFAULT_COVER_ART_PREVIEW_CACHE_ENTRIES = 64;

export type CoverArtPreviewState =
	| { status: 'idle' }
	| { status: 'queued' }
	| { status: 'loading' }
	| { status: 'ready'; bytes: number[]; dataUrl: string }
	| { status: 'error' };

export type CoverArtPreviewLoader = (url: string) => Promise<number[]>;

type CoverArtPreviewSchedulerOptions = {
	failureLogMessage: string;
};

export type CoverArtPreviewScheduler = {
	clear: () => void;
	cancel: () => void;
	getState: (coverUrl: string | null | undefined) => CoverArtPreviewState;
	getCachedBytes: (coverUrl: string) => number[] | null;
	schedule: (
		coverUrls: ReadonlyArray<string | null | undefined>,
		loadCoverArtFromUrl: CoverArtPreviewLoader,
	) => void;
	loadBytes: (coverUrl: string, loadCoverArtFromUrl: CoverArtPreviewLoader) => Promise<number[]>;
	fetch: (coverUrl: string, loadCoverArtFromUrl: CoverArtPreviewLoader) => Promise<void>;
};

export function createCoverArtPreviewScheduler(
	previewByUrl: Record<string, CoverArtPreviewState>,
	options: CoverArtPreviewSchedulerOptions,
): CoverArtPreviewScheduler {
	const inflightByUrl = new Map<string, Promise<number[]>>();
	const cacheOrder: string[] = [];
	const scheduledPreviewQueue = createBoundedGenerationQueue(DEFAULT_COVER_ART_PREVIEW_CONCURRENCY);

	function clear(): void {
		cancel();
		for (const key of Object.keys(previewByUrl)) {
			delete previewByUrl[key];
		}
		cacheOrder.length = 0;
		inflightByUrl.clear();
	}

	function cancel(): void {
		scheduledPreviewQueue.cancel();
		for (const [coverUrl, state] of Object.entries(previewByUrl)) {
			if (state.status === 'queued' || state.status === 'loading') {
				delete previewByUrl[coverUrl];
			}
		}
	}

	function getState(coverUrl: string | null | undefined): CoverArtPreviewState {
		if (!coverUrl) {
			return { status: 'idle' };
		}
		return previewByUrl[coverUrl] ?? { status: 'idle' };
	}

	function getCachedBytes(coverUrl: string): number[] | null {
		const state = previewByUrl[coverUrl];
		return state?.status === 'ready' ? state.bytes : null;
	}

	function schedule(
		coverUrls: ReadonlyArray<string | null | undefined>,
		loadCoverArtFromUrl: CoverArtPreviewLoader,
	): void {
		const uniqueUrls = uniqueCoverUrls(coverUrls);
		scheduledPreviewQueue.schedule(uniqueUrls, {
			visibleKeysChanged: (visibleUrls) => {
				for (const coverUrl of Object.keys(previewByUrl)) {
					if (visibleUrls.has(coverUrl)) {
						continue;
					}
					const state = previewByUrl[coverUrl];
					if (state.status === 'queued' || state.status === 'loading') {
						delete previewByUrl[coverUrl];
					}
				}
			},
			prepare: (coverUrl, generation) => {
				const state = previewByUrl[coverUrl];
				if (state?.status === 'ready') {
					touchCacheEntry(coverUrl);
					return false;
				}
				const inflight = inflightByUrl.get(coverUrl);
				if (inflight) {
					previewByUrl[coverUrl] = { status: 'loading' };
					attachScheduledInflightCompletion(coverUrl, inflight, generation);
					return false;
				}
				previewByUrl[coverUrl] = { status: 'queued' };
				return true;
			},
			start: (coverUrl, generation, complete) =>
				startPreviewFetch(coverUrl, loadCoverArtFromUrl, generation, false, complete),
		});
	}

	async function loadBytes(
		coverUrl: string,
		loadCoverArtFromUrl: CoverArtPreviewLoader,
	): Promise<number[]> {
		const existing = previewByUrl[coverUrl];
		if (existing?.status === 'ready') {
			touchCacheEntry(coverUrl);
			return existing.bytes;
		}
		const inflight = inflightByUrl.get(coverUrl);
		if (inflight) {
			const generation = scheduledPreviewQueue.currentGeneration();
			return inflight.then((bytes) => {
				if (shouldCommitPreviewCompletion(coverUrl, generation, true)) {
					commitReadyPreview(coverUrl, bytes);
				}
				return bytes;
			});
		}
		return scheduledPreviewQueue.track(
			startPreviewFetch(
				coverUrl,
				loadCoverArtFromUrl,
				scheduledPreviewQueue.currentGeneration(),
				true,
			),
		);
	}

	async function fetch(
		coverUrl: string,
		loadCoverArtFromUrl: CoverArtPreviewLoader,
	): Promise<void> {
		await loadBytes(coverUrl, loadCoverArtFromUrl);
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
					console.warn(options.failureLogMessage, error);
					previewByUrl[coverUrl] = { status: 'error' };
					touchCacheEntry(coverUrl);
					prunePreviewCache();
				}
			});
	}

	function startPreviewFetch(
		coverUrl: string,
		loadCoverArtFromUrl: CoverArtPreviewLoader,
		generation: number,
		allowOffscreenCompletion: boolean,
		onComplete?: () => void,
	): Promise<number[]> {
		previewByUrl[coverUrl] = { status: 'loading' };
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
					console.warn(options.failureLogMessage, error);
					previewByUrl[coverUrl] = { status: 'error' };
					touchCacheEntry(coverUrl);
					prunePreviewCache();
				}
				throw error;
			})
			.finally(() => {
				if (inflightByUrl.get(coverUrl) === promise) {
					inflightByUrl.delete(coverUrl);
				}
				onComplete?.();
			});

		inflightByUrl.set(coverUrl, promise);
		return promise;
	}

	function commitReadyPreview(coverUrl: string, bytes: number[]): void {
		previewByUrl[coverUrl] = {
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
		return allowOffscreenCompletion || scheduledPreviewQueue.isCurrent(coverUrl, generation);
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
		while (cacheOrder.length > DEFAULT_COVER_ART_PREVIEW_CACHE_ENTRIES && remainingCandidates > 0) {
			const coverUrl = cacheOrder.shift();
			remainingCandidates -= 1;
			if (!coverUrl) {
				continue;
			}
			if (
				inflightByUrl.has(coverUrl) ||
				scheduledPreviewQueue.isCurrent(coverUrl, scheduledPreviewQueue.currentGeneration())
			) {
				cacheOrder.push(coverUrl);
				continue;
			}
			delete previewByUrl[coverUrl];
		}
	}

	return {
		clear,
		cancel,
		getState,
		getCachedBytes,
		schedule,
		loadBytes,
		fetch,
	};
}
