import { beforeEach, describe, expect, it, vi } from 'vitest';
import { AsyncResult } from '../../../../lib/effect/appEffect';
import { FILE_LIST_COVER_THUMBNAIL_CONCURRENCY } from '../thumbnails';
import { fileListRegistry } from '../session';
import { coverThumbnailAtom, resetFileList, setCoverThumbnailLoader } from '..';

type Deferred<T> = { promise: Promise<T>; resolve: (value: T) => void };

function deferred<T>(): Deferred<T> {
	let resolve!: (value: T) => void;
	const promise = new Promise<T>((resolvePromise) => {
		resolve = resolvePromise;
	});
	return { promise, resolve };
}

async function flush(): Promise<void> {
	for (let i = 0; i < 20; i += 1) await Promise.resolve();
}

function thumbnail(path: string) {
	return fileListRegistry.get(coverThumbnailAtom(path));
}

describe('Solid File List cover thumbnail atoms', () => {
	beforeEach(() => {
		resetFileList();
	});

	it('bounds loading to two paths and dedupes duplicate family mounts', async () => {
		const requests: Array<Deferred<number[] | null>> = [];
		const load = vi.fn((_path: string, _signal: AbortSignal) => {
			const request = deferred<number[] | null>();
			requests.push(request);
			return request.promise;
		});
		setCoverThumbnailLoader(load);
		fileListRegistry.mount(coverThumbnailAtom('/a'));
		fileListRegistry.mount(coverThumbnailAtom('/a'));
		fileListRegistry.mount(coverThumbnailAtom('/b'));
		fileListRegistry.mount(coverThumbnailAtom('/c'));
		await flush();
		expect(load).toHaveBeenCalledTimes(FILE_LIST_COVER_THUMBNAIL_CONCURRENCY);
		expect(AsyncResult.isWaiting(thumbnail('/c')) || AsyncResult.isInitial(thumbnail('/c'))).toBe(
			true,
		);
		requests[0]?.resolve([1]);
		await flush();
		expect(load).toHaveBeenCalledTimes(3);
		requests[1]?.resolve([2]);
		requests[2]?.resolve([3]);
		await flush();
		expect(AsyncResult.isSuccess(thumbnail('/a'))).toBe(true);
	});

	it('ignores stale completion after reset and re-mount', async () => {
		const stale = deferred<number[]>();
		const fresh = deferred<number[]>();
		const load = vi.fn().mockReturnValueOnce(stale.promise).mockReturnValueOnce(fresh.promise);
		setCoverThumbnailLoader(load);
		const unmount = fileListRegistry.mount(coverThumbnailAtom('/book'));
		await flush();
		unmount();
		resetFileList();
		setCoverThumbnailLoader(load);
		fileListRegistry.mount(coverThumbnailAtom('/book'));
		await flush();
		stale.resolve([1]);
		await flush();
		expect(AsyncResult.isSuccess(thumbnail('/book')) && thumbnail('/book').waiting).toBeFalsy();
		expect(
			AsyncResult.isWaiting(thumbnail('/book')) || AsyncResult.isInitial(thumbnail('/book')),
		).toBe(true);
		fresh.resolve([2]);
		await flush();
		const result = thumbnail('/book');
		expect(AsyncResult.isSuccess(result)).toBe(true);
		if (AsyncResult.isSuccess(result)) {
			expect(result.value).toBe('data:image/jpeg;base64,Ag==');
		}
	});
});
