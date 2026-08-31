import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
	clearFileListCoverThumbnails,
	MAX_FILE_LIST_COVER_THUMBNAIL_CACHE_ENTRIES,
	getFileListCoverThumbnailState,
	scheduleFileListCoverThumbnails,
} from '../coverThumbnails';

type Deferred<T> = { promise: Promise<T>; resolve: (value: T) => void };
function deferred<T>(): Deferred<T> {
	let resolve!: (value: T) => void;
	const promise = new Promise<T>((resolvePromise) => {
		resolve = resolvePromise;
	});
	return { promise, resolve };
}
async function flush(): Promise<void> {
	for (let i = 0; i < 12; i += 1) await Promise.resolve();
}

describe('FileList cover thumbnail scheduler', () => {
	beforeEach(() => clearFileListCoverThumbnails());

	it('bounds loading to two paths and dedupes duplicate paths', async () => {
		const requests: Array<Deferred<string | null>> = [];
		const load = vi.fn(() => {
			const request = deferred<string | null>();
			requests.push(request);
			return request.promise;
		});
		scheduleFileListCoverThumbnails(['/a', '/a', '/b', '/c'], load);
		await flush();
		expect(load).toHaveBeenCalledTimes(2);
		expect(getFileListCoverThumbnailState('/c').status).toBe('queued');
		requests[0].resolve('data:image/jpeg;base64,a');
		await flush();
		expect(load).toHaveBeenCalledTimes(3);
		requests[1].resolve('data:image/jpeg;base64,b');
		requests[2].resolve('data:image/jpeg;base64,c');
		await flush();
	});

	it('ignores stale completion after clear and re-add', async () => {
		const stale = deferred<string>();
		const fresh = deferred<string>();
		const load = vi.fn().mockReturnValueOnce(stale.promise).mockReturnValueOnce(fresh.promise);
		scheduleFileListCoverThumbnails(['/book'], load);
		await flush();
		clearFileListCoverThumbnails();
		scheduleFileListCoverThumbnails(['/book'], load);
		await flush();
		stale.resolve('data:image/jpeg;base64,stale');
		await flush();
		expect(getFileListCoverThumbnailState('/book').status).toBe('loading');
		fresh.resolve('data:image/jpeg;base64,fresh');
		await flush();
		expect(getFileListCoverThumbnailState('/book')).toMatchObject({ status: 'ready' });
	});

	it('keeps the terminal thumbnail cache bounded even when every path is visible', async () => {
		const paths = Array.from(
			{ length: MAX_FILE_LIST_COVER_THUMBNAIL_CACHE_ENTRIES + 1 },
			(_, index) => `/book-${index}`,
		);
		scheduleFileListCoverThumbnails(paths, async () => 'data:image/jpeg;base64,thumb');
		for (let index = 0; index < 150; index += 1) await Promise.resolve();

		expect(getFileListCoverThumbnailState(paths[0]).status).toBe('idle');
		expect(getFileListCoverThumbnailState(paths[paths.length - 1]).status).toBe('ready');
	});
});
