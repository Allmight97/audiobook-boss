import { createBoundedGenerationQueue } from './boundedGenerationQueue';

export const DEFAULT_COVER_ART_PREVIEW_CONCURRENCY = 2;
export const DEFAULT_COVER_ART_PREVIEW_CACHE_ENTRIES = 64;

export type CoverArtPreviewState =
	| { status: 'idle' }
	| { status: 'queued' }
	| { status: 'loading' }
	| { status: 'ready'; dataUrl: string }
	| { status: 'error' };

export type CoverArtPreviewLoader = (url: string) => Promise<string>;

type CoverArtPreviewSchedulerOptions = {
	failureLogMessage: string;
};

export type CoverArtPreviewScheduler = {
	clear: () => void;
	cancel: () => void;
	getState: (coverUrl: string | null | undefined) => CoverArtPreviewState;
	schedule: (
		coverUrls: ReadonlyArray<string | null | undefined>,
		loadCoverPreview: CoverArtPreviewLoader,
	) => void;
	fetch: (coverUrl: string, loadCoverPreview: CoverArtPreviewLoader) => Promise<void>;
};

export function createCoverArtPreviewScheduler(
	previewByUrl: Record<string, CoverArtPreviewState>,
	options: CoverArtPreviewSchedulerOptions,
): CoverArtPreviewScheduler {
	const inflightByUrl = new Map<string, Promise<string>>();
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

	function schedule(
		coverUrls: ReadonlyArray<string | null | undefined>,
		loadCoverPreview: CoverArtPreviewLoader,
	): void {
		const uniqueUrls = uniqueCoverUrls(coverUrls);
		scheduledPreviewQueue.schedule(uniqueUrls, {
			visibleKeysChanged: (visibleUrls) => {
				for (const coverUrl of Object.keys(previewByUrl)) {
					if (visibleUrls.has(coverUrl)) continue;
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
				startScheduledPreviewFetch(coverUrl, loadCoverPreview, generation, complete),
		});
	}

	async function fetch(coverUrl: string, loadCoverPreview: CoverArtPreviewLoader): Promise<void> {
		const existing = previewByUrl[coverUrl];
		if (existing?.status === 'ready') {
			touchCacheEntry(coverUrl);
			return;
		}
		const inflight = inflightByUrl.get(coverUrl);
		if (inflight) {
			const generation = scheduledPreviewQueue.currentGeneration();
			await inflight.then((dataUrl) => {
				if (shouldCommitPreviewCompletion(coverUrl, generation, true)) {
					commitReadyPreview(coverUrl, dataUrl);
				}
			});
			return;
		}
		await scheduledPreviewQueue.track(
			startPreviewFetch(
				coverUrl,
				loadCoverPreview,
				scheduledPreviewQueue.currentGeneration(),
				true,
			),
		);
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
		inflight: Promise<string>,
		generation: number,
	): void {
		void inflight
			.then((dataUrl) => {
				if (shouldCommitPreviewCompletion(coverUrl, generation, false)) {
					commitReadyPreview(coverUrl, dataUrl);
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

	function startScheduledPreviewFetch(
		coverUrl: string,
		loadCoverPreview: CoverArtPreviewLoader,
		generation: number,
		onComplete: () => void,
	): Promise<string> {
		const existing = previewByUrl[coverUrl];
		if (existing?.status === 'ready') {
			touchCacheEntry(coverUrl);
			onComplete();
			return Promise.resolve(existing.dataUrl);
		}
		const inflight = inflightByUrl.get(coverUrl);
		if (inflight) {
			attachScheduledInflightCompletion(coverUrl, inflight, generation);
			return inflight.finally(onComplete);
		}
		return startPreviewFetch(coverUrl, loadCoverPreview, generation, false, onComplete);
	}

	function startPreviewFetch(
		coverUrl: string,
		loadCoverPreview: CoverArtPreviewLoader,
		generation: number,
		allowOffscreenCompletion: boolean,
		onComplete?: () => void,
	): Promise<string> {
		previewByUrl[coverUrl] = { status: 'loading' };
		let promise!: Promise<string>;
		promise = loadCoverPreview(coverUrl)
			.then((dataUrl): string => {
				if (shouldCommitPreviewCompletion(coverUrl, generation, allowOffscreenCompletion)) {
					commitReadyPreview(coverUrl, dataUrl);
				}
				return dataUrl;
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

	function commitReadyPreview(coverUrl: string, dataUrl: string): void {
		previewByUrl[coverUrl] = {
			status: 'ready',
			dataUrl,
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
		schedule,
		fetch,
	};
}
