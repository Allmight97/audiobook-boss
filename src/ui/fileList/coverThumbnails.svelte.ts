import { coverArtBytesToDataUrl } from '../../lib/media/coverArtDataUrl';
import {
	createBoundedGenerationQueue,
	type BoundedGenerationQueue,
} from '../../lib/media/boundedGenerationQueue';
import { tauriClient } from '../../lib/tauri/client';

export const FILE_LIST_COVER_THUMBNAIL_CONCURRENCY = 2;
/** Visible paths are retained even when their count temporarily exceeds this cap. */
export const MAX_FILE_LIST_COVER_THUMBNAIL_CACHE_ENTRIES = 64;

export type FileListCoverThumbnailState =
	| { status: 'idle' }
	| { status: 'queued' }
	| { status: 'loading' }
	| { status: 'ready'; dataUrl: string }
	| { status: 'absent' }
	| { status: 'error' };

export type FileListCoverThumbnailLoader = (
	path: string,
) => Promise<ReadonlyArray<number> | null | undefined>;

const thumbnailByPath = $state<Record<string, FileListCoverThumbnailState>>({});
const inflightByPath = new Map<string, Promise<ReadonlyArray<number> | null | undefined>>();
const cacheOrder: string[] = [];
const thumbnailQueue: BoundedGenerationQueue = createBoundedGenerationQueue(
	FILE_LIST_COVER_THUMBNAIL_CONCURRENCY,
);

function defaultLoader(path: string): Promise<ReadonlyArray<number> | null | undefined> {
	return tauriClient.readAudioCoverThumbnail(path);
}

export function clearFileListCoverThumbnails(): void {
	thumbnailQueue.cancel();
	for (const path of Object.keys(thumbnailByPath)) {
		delete thumbnailByPath[path];
	}
	cacheOrder.length = 0;
}

export function getFileListCoverThumbnailState(
	path: string | null | undefined,
): FileListCoverThumbnailState {
	// Read the reactive map before checking the non-reactive queue generation so
	// a row rendered before scheduling still updates when its thumbnail arrives.
	const state = path ? thumbnailByPath[path] : undefined;
	if (!path || !thumbnailQueue.isCurrent(path, thumbnailQueue.currentGeneration())) {
		return { status: 'idle' };
	}
	return state ?? { status: 'idle' };
}

export function scheduleFileListCoverThumbnails(
	paths: ReadonlyArray<string>,
	loadThumbnail: FileListCoverThumbnailLoader = defaultLoader,
): void {
	thumbnailQueue.schedule(paths, {
		visibleKeysChanged: (visiblePaths) => {
			for (const path of Object.keys(thumbnailByPath)) {
				if (!visiblePaths.has(path)) {
					const state = thumbnailByPath[path];
					if (state.status === 'queued' || state.status === 'loading') {
						delete thumbnailByPath[path];
					}
				}
			}
		},
		prepare: (path, generation) => {
			const state = thumbnailByPath[path];
			if (isTerminalThumbnailState(state)) {
				touchCacheEntry(path);
				return false;
			}
			const inflight = inflightByPath.get(path);
			if (inflight) {
				thumbnailByPath[path] = { status: 'loading' };
				attachInflightCompletion(path, inflight, generation);
				return false;
			}
			thumbnailByPath[path] = { status: 'queued' };
			return true;
		},
		start: (path, generation, complete) =>
			startThumbnailFetch(path, loadThumbnail, generation, complete),
	});
}

function startThumbnailFetch(
	path: string,
	loadThumbnail: FileListCoverThumbnailLoader,
	generation: number,
	onComplete: () => void,
): Promise<ReadonlyArray<number> | null | undefined> {
	thumbnailByPath[path] = { status: 'loading' };
	let promise!: Promise<ReadonlyArray<number> | null | undefined>;
	const load = loadThumbnail(path);
	promise = load
		.then((bytes) => {
			if (thumbnailQueue.isCurrent(path, generation)) {
				commitThumbnail(path, bytes);
			}
			return bytes;
		})
		.catch((error) => {
			if (thumbnailQueue.isCurrent(path, generation)) {
				console.warn('Failed to load file-list cover thumbnail:', error);
				thumbnailByPath[path] = { status: 'error' };
				touchCacheEntry(path);
				pruneThumbnailCache();
			}
			throw error;
		})
		.finally(() => {
			if (inflightByPath.get(path) === promise) {
				inflightByPath.delete(path);
			}
			onComplete();
		});
	inflightByPath.set(path, promise);
	return promise;
}

function attachInflightCompletion(
	path: string,
	inflight: Promise<ReadonlyArray<number> | null | undefined>,
	generation: number,
): void {
	void inflight
		.then((bytes) => {
			if (thumbnailQueue.isCurrent(path, generation)) {
				commitThumbnail(path, bytes);
			}
		})
		.catch((error) => {
			if (thumbnailQueue.isCurrent(path, generation)) {
				console.warn('Failed to load file-list cover thumbnail:', error);
				thumbnailByPath[path] = { status: 'error' };
				touchCacheEntry(path);
				pruneThumbnailCache();
			}
		});
}

function commitThumbnail(path: string, bytes: ReadonlyArray<number> | null | undefined): void {
	thumbnailByPath[path] = bytes
		? { status: 'ready', dataUrl: coverArtBytesToDataUrl(Array.from(bytes)) }
		: { status: 'absent' };
	touchCacheEntry(path);
	pruneThumbnailCache();
}

function isTerminalThumbnailState(
	state: FileListCoverThumbnailState | undefined,
): state is Extract<FileListCoverThumbnailState, { status: 'ready' | 'absent' | 'error' }> {
	return state?.status === 'ready' || state?.status === 'absent' || state?.status === 'error';
}

function touchCacheEntry(path: string): void {
	const existingIndex = cacheOrder.indexOf(path);
	if (existingIndex >= 0) {
		cacheOrder.splice(existingIndex, 1);
	}
	cacheOrder.push(path);
}

function pruneThumbnailCache(): void {
	let remainingCandidates = cacheOrder.length;
	while (
		cacheOrder.length > MAX_FILE_LIST_COVER_THUMBNAIL_CACHE_ENTRIES &&
		remainingCandidates > 0
	) {
		const path = cacheOrder.shift();
		remainingCandidates -= 1;
		if (!path) {
			continue;
		}
		if (thumbnailQueue.isCurrent(path, thumbnailQueue.currentGeneration())) {
			cacheOrder.push(path);
			continue;
		}
		delete thumbnailByPath[path];
	}
}
