import { createRoot, createStore, flush } from 'solid-js';
import { coverArtBytesToDataUrl } from '../../../lib/media/coverArtDataUrl';
import { createBoundedGenerationQueue } from '../../../lib/media/boundedGenerationQueue';
import { tauriClient } from '../../../lib/tauri/client';

export const FILE_LIST_COVER_THUMBNAIL_CONCURRENCY = 2;
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

const graph = createRoot(() => createStore<Record<string, FileListCoverThumbnailState>>({}));
const [thumbnailByPath, setThumbnailByPath] = graph;
const inflightByPath = new Map<string, Promise<ReadonlyArray<number> | null | undefined>>();
const cacheOrder: string[] = [];
const thumbnailQueue = createBoundedGenerationQueue(FILE_LIST_COVER_THUMBNAIL_CONCURRENCY);
let loadThumbnail: FileListCoverThumbnailLoader = (path) =>
	tauriClient.readAudioCoverThumbnail(path);

function put(path: string, state: FileListCoverThumbnailState): void {
	setThumbnailByPath((draft) => {
		draft[path] = state;
	});
	flush();
}

export function setCoverThumbnailLoader(loader: FileListCoverThumbnailLoader): void {
	loadThumbnail = loader;
}

export function resetCoverThumbnailRuntime(): void {
	clearFileListCoverThumbnails();
	loadThumbnail = (path) => tauriClient.readAudioCoverThumbnail(path);
}

export function clearFileListCoverThumbnails(): void {
	thumbnailQueue.cancel();
	setThumbnailByPath((draft) => {
		for (const path of Object.keys(draft)) delete draft[path];
	});
	cacheOrder.length = 0;
	inflightByPath.clear();
	flush();
}

export function removeFileListCoverThumbnail(path: string): void {
	setThumbnailByPath((draft) => {
		delete draft[path];
	});
	const cacheIndex = cacheOrder.indexOf(path);
	if (cacheIndex >= 0) cacheOrder.splice(cacheIndex, 1);
	inflightByPath.delete(path);
	flush();
}

export function getFileListCoverThumbnailState(
	path: string | null | undefined,
): FileListCoverThumbnailState {
	const state = path ? thumbnailByPath[path] : undefined;
	if (!path || !thumbnailQueue.isCurrent(path, thumbnailQueue.currentGeneration())) {
		return { status: 'idle' };
	}
	return state ?? { status: 'idle' };
}

export function scheduleFileListCoverThumbnails(paths: ReadonlyArray<string>): void {
	thumbnailQueue.schedule(paths, {
		visibleKeysChanged: (visiblePaths) => {
			setThumbnailByPath((draft) => {
				for (const path of Object.keys(draft)) {
					if (visiblePaths.has(path)) continue;
					const state = draft[path];
					if (state?.status === 'queued' || state?.status === 'loading') delete draft[path];
				}
			});
			flush();
		},
		prepare: (path, generation) => {
			const state = thumbnailByPath[path];
			if (isTerminal(state)) {
				touch(path);
				return false;
			}
			const inflight = inflightByPath.get(path);
			if (inflight) {
				put(path, { status: 'loading' });
				attachInflight(path, inflight, generation);
				return false;
			}
			put(path, { status: 'queued' });
			return true;
		},
		start: (path, generation, complete) => startFetch(path, generation, complete),
	});
}

function startFetch(
	path: string,
	generation: number,
	onComplete: () => void,
): Promise<ReadonlyArray<number> | null | undefined> {
	put(path, { status: 'loading' });
	let promise!: Promise<ReadonlyArray<number> | null | undefined>;
	promise = loadThumbnail(path)
		.then((bytes) => {
			if (thumbnailQueue.isCurrent(path, generation)) commit(path, bytes);
			return bytes;
		})
		.catch((error) => {
			if (thumbnailQueue.isCurrent(path, generation)) {
				put(path, { status: 'error' });
				touch(path);
				prune();
			}
			throw error;
		})
		.finally(() => {
			if (inflightByPath.get(path) === promise) inflightByPath.delete(path);
			onComplete();
		});
	inflightByPath.set(path, promise);
	return promise;
}

function attachInflight(
	path: string,
	inflight: Promise<ReadonlyArray<number> | null | undefined>,
	generation: number,
): void {
	void inflight
		.then((bytes) => {
			if (thumbnailQueue.isCurrent(path, generation)) commit(path, bytes);
		})
		.catch(() => {
			if (thumbnailQueue.isCurrent(path, generation)) {
				put(path, { status: 'error' });
				touch(path);
				prune();
			}
		});
}

function commit(path: string, bytes: ReadonlyArray<number> | null | undefined): void {
	put(
		path,
		bytes
			? { status: 'ready', dataUrl: coverArtBytesToDataUrl(Array.from(bytes)) }
			: { status: 'absent' },
	);
	touch(path);
	prune();
}

function isTerminal(
	state: FileListCoverThumbnailState | undefined,
): state is Extract<FileListCoverThumbnailState, { status: 'ready' | 'absent' | 'error' }> {
	return state?.status === 'ready' || state?.status === 'absent' || state?.status === 'error';
}

function touch(path: string): void {
	const index = cacheOrder.indexOf(path);
	if (index >= 0) cacheOrder.splice(index, 1);
	cacheOrder.push(path);
}

function prune(): void {
	while (cacheOrder.length > MAX_FILE_LIST_COVER_THUMBNAIL_CACHE_ENTRIES) {
		const path = cacheOrder.shift();
		if (path) {
			setThumbnailByPath((draft) => {
				delete draft[path];
			});
		}
	}
}
