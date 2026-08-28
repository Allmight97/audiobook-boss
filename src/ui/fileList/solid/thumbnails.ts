import { coverArtBytesToDataUrl } from '../../../lib/media/coverArtDataUrl';
import { tauriClient } from '../../../lib/tauri/client';
import { Atom, Effect } from '../../../lib/effect/appEffect';
import { fileListRegistry } from './session';

export const FILE_LIST_COVER_THUMBNAIL_CONCURRENCY = 2;

export type CoverThumbnailLoader = (
	path: string,
	signal: AbortSignal,
) => Promise<ReadonlyArray<number> | null | undefined>;

type CoverThumbnailLoaderState = { readonly load: CoverThumbnailLoader };

const defaultLoader: CoverThumbnailLoader = async (path) =>
	tauriClient.readAudioCoverThumbnail(path);

const INITIAL_LOADER: CoverThumbnailLoaderState = { load: defaultLoader };

export const coverThumbnailLoaderAtom = Atom.make<CoverThumbnailLoaderState>(INITIAL_LOADER).pipe(
	Atom.keepAlive,
);

let activeCount = 0;
const waiters: Array<() => void> = [];

function acquireSlot(signal: AbortSignal): Promise<void> {
	return new Promise((resolve, reject) => {
		const abort = (): void => {
			const index = waiters.indexOf(start);
			if (index >= 0) waiters.splice(index, 1);
			reject(Object.assign(new Error('aborted'), { name: 'AbortError' }));
		};
		const start = (): void => {
			signal.removeEventListener('abort', abort);
			activeCount += 1;
			resolve();
		};
		if (signal.aborted) {
			abort();
			return;
		}
		signal.addEventListener('abort', abort, { once: true });
		if (activeCount < FILE_LIST_COVER_THUMBNAIL_CONCURRENCY) {
			start();
			return;
		}
		waiters.push(start);
	});
}

function releaseSlot(): void {
	activeCount = Math.max(0, activeCount - 1);
	const next = waiters.shift();
	if (next) next();
}

export function setCoverThumbnailLoader(loader: CoverThumbnailLoader): void {
	fileListRegistry.set(coverThumbnailLoaderAtom, { load: loader });
}

export function resetCoverThumbnailRuntime(): void {
	activeCount = 0;
	waiters.length = 0;
	fileListRegistry.set(coverThumbnailLoaderAtom, INITIAL_LOADER);
}

export const coverThumbnailAtom = Atom.family((path: string) =>
	Atom.make(() =>
		Effect.tryPromise({
			try: async (signal) => {
				const loader = fileListRegistry.get(coverThumbnailLoaderAtom).load;
				await acquireSlot(signal);
				try {
					const bytes = await loader(path, signal);
					if (signal.aborted) {
						throw Object.assign(new Error('aborted'), { name: 'AbortError' });
					}
					return bytes ? coverArtBytesToDataUrl(Array.from(bytes)) : null;
				} finally {
					releaseSlot();
				}
			},
			catch: (cause) => cause,
		}),
	),
);
