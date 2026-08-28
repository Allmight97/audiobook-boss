import { coverArtBytesToDataUrl } from '../../../lib/media/coverArtDataUrl';
import { tauriClient } from '../../../lib/tauri/client';
import { Atom, Effect } from '../../../lib/effect/appEffect';
import { fileListRegistry } from './session';

export const FILE_LIST_COVER_THUMBNAIL_CONCURRENCY = 2;

export type CoverThumbnailLoader = (
	path: string,
	signal: AbortSignal,
) => Promise<ReadonlyArray<number> | null | undefined>;

const defaultLoader: CoverThumbnailLoader = async (path) =>
	tauriClient.readAudioCoverThumbnail(path);

export const coverThumbnailLoaderAtom = Atom.make<CoverThumbnailLoader>(defaultLoader).pipe(
	Atom.keepAlive,
);

let activeCount = 0;
const waiters: Array<() => void> = [];

function acquireSlot(): Effect.Effect<void> {
	return Effect.suspend(() => {
		if (activeCount < FILE_LIST_COVER_THUMBNAIL_CONCURRENCY) {
			activeCount += 1;
			return Effect.succeed(undefined);
		}
		return Effect.callback<void>((resume, signal) => {
			const wake = (): void => {
				if (signal.aborted) return;
				activeCount += 1;
				resume(Effect.succeed(undefined));
			};
			waiters.push(wake);
			signal.addEventListener(
				'abort',
				() => {
					const index = waiters.indexOf(wake);
					if (index >= 0) waiters.splice(index, 1);
				},
				{ once: true },
			);
		});
	});
}

function releaseSlot(): void {
	activeCount = Math.max(0, activeCount - 1);
	const next = waiters.shift();
	if (next) next();
}

export function setCoverThumbnailLoader(loader: CoverThumbnailLoader): void {
	fileListRegistry.set(coverThumbnailLoaderAtom, loader);
}

export function resetCoverThumbnailRuntime(): void {
	activeCount = 0;
	waiters.length = 0;
	fileListRegistry.set(coverThumbnailLoaderAtom, defaultLoader);
}

export const coverThumbnailAtom = Atom.family((path: string) =>
	Atom.make((get) => {
		const loader = get(coverThumbnailLoaderAtom);
		return Effect.acquireRelease(acquireSlot(), () => Effect.sync(releaseSlot)).pipe(
			Effect.flatMap(() =>
				Effect.tryPromise({
					try: (signal) => loader(path, signal),
					catch: (cause) => cause,
				}),
			),
			Effect.map((bytes) => (bytes ? coverArtBytesToDataUrl(Array.from(bytes)) : null)),
		);
	}),
);
