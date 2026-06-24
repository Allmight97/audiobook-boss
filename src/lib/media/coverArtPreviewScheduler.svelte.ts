import { coverArtBytesToDataUrl } from './coverArtDataUrl';

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
	maxConcurrency?: number;
	maxCacheEntries?: number;
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
	const maxConcurrency = options.maxConcurrency ?? DEFAULT_COVER_ART_PREVIEW_CONCURRENCY;
	const maxCacheEntries = options.maxCacheEntries ?? DEFAULT_COVER_ART_PREVIEW_CACHE_ENTRIES;
	const inflightByUrl = new Map<string, Promise<number[]>>();
	const activePreviewTasks = new Set<number>();
	const cacheOrder: string[] = [];

	let nextPreviewTaskId = 0;
	let previewScheduleGeneration = 0;
	let queuedCoverUrls: string[] = [];
	let visibleCoverUrls = new Set<string>();
	let activeLoader: CoverArtPreviewLoader | null = null;

	function clear(): void {
		cancel();
		for (const key of Object.keys(previewByUrl)) {
			delete previewByUrl[key];
		}
		cacheOrder.length = 0;
		inflightByUrl.clear();
	}

	function cancel(): void {
		previewScheduleGeneration += 1;
		queuedCoverUrls = [];
		visibleCoverUrls = new Set();
		activeLoader = null;
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
		previewScheduleGeneration += 1;
		activeLoader = loadCoverArtFromUrl;
		const generation = previewScheduleGeneration;
		const uniqueUrls = uniqueCoverUrls(coverUrls);
		visibleCoverUrls = new Set(uniqueUrls);
		queuedCoverUrls = [];

		for (const coverUrl of Object.keys(previewByUrl)) {
			if (!visibleCoverUrls.has(coverUrl)) {
				const state = previewByUrl[coverUrl];
				if (state.status === 'queued' || state.status === 'loading') {
					delete previewByUrl[coverUrl];
				}
			}
		}

		for (const coverUrl of uniqueUrls) {
			const state = previewByUrl[coverUrl];
			if (state?.status === 'ready') {
				touchCacheEntry(coverUrl);
				continue;
			}
			const inflight = inflightByUrl.get(coverUrl);
			if (inflight) {
				previewByUrl[coverUrl] = { status: 'loading' };
				attachScheduledInflightCompletion(coverUrl, inflight, generation);
				continue;
			}
			previewByUrl[coverUrl] = { status: 'queued' };
			queuedCoverUrls.push(coverUrl);
		}

		pumpPreviewQueue(generation);
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

	function pumpPreviewQueue(generation: number): void {
		if (!activeLoader || generation !== previewScheduleGeneration) {
			return;
		}
		while (activePreviewTasks.size < maxConcurrency && queuedCoverUrls.length > 0) {
			const coverUrl = queuedCoverUrls.shift();
			if (!coverUrl || !visibleCoverUrls.has(coverUrl)) {
				continue;
			}
			const state = previewByUrl[coverUrl];
			if (state?.status === 'ready' || inflightByUrl.has(coverUrl)) {
				continue;
			}
			void startPreviewFetch(coverUrl, activeLoader, generation, false).catch(() => undefined);
		}
	}

	function startPreviewFetch(
		coverUrl: string,
		loadCoverArtFromUrl: CoverArtPreviewLoader,
		generation: number,
		allowOffscreenCompletion: boolean,
	): Promise<number[]> {
		previewByUrl[coverUrl] = { status: 'loading' };
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
					console.warn(options.failureLogMessage, error);
					previewByUrl[coverUrl] = { status: 'error' };
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
		while (cacheOrder.length > maxCacheEntries && remainingCandidates > 0) {
			const coverUrl = cacheOrder.shift();
			remainingCandidates -= 1;
			if (!coverUrl) {
				continue;
			}
			if (inflightByUrl.has(coverUrl) || visibleCoverUrls.has(coverUrl)) {
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
