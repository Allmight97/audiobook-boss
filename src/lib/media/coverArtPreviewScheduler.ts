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
	readonly load: CoverArtPreviewLoader;
	/** Called after every preview state change so owners can publish. */
	readonly onChange: () => void;
	readonly failureLogMessage: string;
};

/** One owner instance's preview cache; each owner creates its own. */
export type CoverArtPreviewScheduler = {
	clear: () => void;
	cancel: () => void;
	getState: (coverUrl: string | null | undefined) => CoverArtPreviewState;
	schedule: (coverUrls: ReadonlyArray<string | null | undefined>) => void;
	/** Returns the URL's bytes, joining a scheduled request already in flight. */
	loadBytes: (coverUrl: string) => Promise<number[]>;
};

export function createCoverArtPreviewScheduler(
	options: CoverArtPreviewSchedulerOptions,
): CoverArtPreviewScheduler {
	const previewByUrl: Record<string, CoverArtPreviewState> = {};
	const loadCoverArtFromUrl = options.load;
	const inflightByUrl = new Map<string, Promise<number[]>>();
	const cacheOrder: string[] = [];
	const scheduledPreviewQueue = createBoundedGenerationQueue(DEFAULT_COVER_ART_PREVIEW_CONCURRENCY);

	function setPreview(coverUrl: string, state: CoverArtPreviewState): void {
		previewByUrl[coverUrl] = state;
		options.onChange();
	}

	function deletePreview(coverUrl: string): void {
		delete previewByUrl[coverUrl];
		options.onChange();
	}

	function clear(): void {
		cancel();
		for (const key of Object.keys(previewByUrl)) {
			deletePreview(key);
		}
		cacheOrder.length = 0;
		inflightByUrl.clear();
	}

	function cancel(): void {
		scheduledPreviewQueue.cancel();
		for (const [coverUrl, state] of Object.entries(previewByUrl)) {
			if (state.status === 'queued' || state.status === 'loading') {
				deletePreview(coverUrl);
			}
		}
	}

	function getState(coverUrl: string | null | undefined): CoverArtPreviewState {
		if (!coverUrl) {
			return { status: 'idle' };
		}
		return previewByUrl[coverUrl] ?? { status: 'idle' };
	}

	function schedule(coverUrls: ReadonlyArray<string | null | undefined>): void {
		const uniqueUrls = uniqueCoverUrls(coverUrls);
		scheduledPreviewQueue.schedule(uniqueUrls, {
			visibleKeysChanged: (visibleUrls) => {
				for (const coverUrl of Object.keys(previewByUrl)) {
					if (visibleUrls.has(coverUrl)) continue;
					const state = previewByUrl[coverUrl];
					if (state.status === 'queued' || state.status === 'loading') {
						deletePreview(coverUrl);
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
					setPreview(coverUrl, { status: 'loading' });
					attachScheduledInflightCompletion(coverUrl, inflight, generation);
					return false;
				}
				setPreview(coverUrl, { status: 'queued' });
				return true;
			},
			start: (coverUrl, generation, complete) =>
				startScheduledPreviewFetch(coverUrl, generation, complete),
		});
	}

	async function loadBytes(coverUrl: string): Promise<number[]> {
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
			startPreviewFetch(coverUrl, scheduledPreviewQueue.currentGeneration(), true),
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
					setPreview(coverUrl, { status: 'error' });
					touchCacheEntry(coverUrl);
					prunePreviewCache();
				}
			});
	}

	function startScheduledPreviewFetch(
		coverUrl: string,
		generation: number,
		onComplete: () => void,
	): Promise<number[]> {
		const existing = previewByUrl[coverUrl];
		if (existing?.status === 'ready') {
			touchCacheEntry(coverUrl);
			onComplete();
			return Promise.resolve(existing.bytes);
		}
		const inflight = inflightByUrl.get(coverUrl);
		if (inflight) {
			attachScheduledInflightCompletion(coverUrl, inflight, generation);
			return inflight.finally(onComplete);
		}
		return startPreviewFetch(coverUrl, generation, false, onComplete);
	}

	function startPreviewFetch(
		coverUrl: string,
		generation: number,
		allowOffscreenCompletion: boolean,
		onComplete?: () => void,
	): Promise<number[]> {
		setPreview(coverUrl, { status: 'loading' });
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
					setPreview(coverUrl, { status: 'error' });
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
		setPreview(coverUrl, { status: 'ready', bytes, dataUrl: coverArtBytesToDataUrl(bytes) });
		touchCacheEntry(coverUrl);
		prunePreviewCache();
	}

	/**
	 * Scheduled loads commit only while their URL is still visible. Apply's
	 * direct loads may commit offscreen, but never after a cancel, clear, or
	 * reschedule has started a newer generation.
	 */
	function shouldCommitPreviewCompletion(
		coverUrl: string,
		generation: number,
		allowOffscreenCompletion: boolean,
	): boolean {
		if (allowOffscreenCompletion) {
			return generation === scheduledPreviewQueue.currentGeneration();
		}
		return scheduledPreviewQueue.isCurrent(coverUrl, generation);
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
			deletePreview(coverUrl);
		}
	}

	return { clear, cancel, getState, schedule, loadBytes };
}
