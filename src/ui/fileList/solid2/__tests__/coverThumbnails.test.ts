import { beforeEach, describe, expect, it, vi } from 'vitest';
import { resetFileList, setCoverThumbnailLoader } from '..';
import {
	FILE_LIST_COVER_THUMBNAIL_CONCURRENCY,
	MAX_FILE_LIST_COVER_THUMBNAIL_CACHE_ENTRIES,
	getFileListCoverThumbnailState,
	removeFileListCoverThumbnail,
	scheduleFileListCoverThumbnails,
} from '../thumbnails';

type Deferred<T> = { promise: Promise<T>; resolve: (value: T) => void };

function deferred<T>(): Deferred<T> {
	let resolve!: (value: T) => void;
	const promise = new Promise<T>((resolvePromise) => {
		resolve = resolvePromise;
	});
	return { promise, resolve };
}

async function flushMicrotasks(): Promise<void> {
	for (let i = 0; i < 20; i += 1) await Promise.resolve();
}

describe('Solid 2 File List cover thumbnails', () => {
	beforeEach(() => {
		resetFileList();
	});

	it('bounds loading to two paths and dedupes duplicate paths', async () => {
		const requests: Array<Deferred<number[] | null>> = [];
		const load = vi.fn(() => {
			const request = deferred<number[] | null>();
			requests.push(request);
			return request.promise;
		});
		setCoverThumbnailLoader(load);
		scheduleFileListCoverThumbnails(['/a', '/a', '/b', '/c']);
		await flushMicrotasks();
		expect(load).toHaveBeenCalledTimes(FILE_LIST_COVER_THUMBNAIL_CONCURRENCY);
		expect(getFileListCoverThumbnailState('/c').status).toBe('queued');
		requests[0]?.resolve([1]);
		await flushMicrotasks();
		expect(load).toHaveBeenCalledTimes(3);
		requests[1]?.resolve([2]);
		requests[2]?.resolve([3]);
		await flushMicrotasks();
		expect(getFileListCoverThumbnailState('/a')).toMatchObject({ status: 'ready' });
	});

	it('ignores stale completion after clear and re-add', async () => {
		const stale = deferred<number[]>();
		const fresh = deferred<number[]>();
		const load = vi.fn().mockReturnValueOnce(stale.promise).mockReturnValueOnce(fresh.promise);
		setCoverThumbnailLoader(load);
		scheduleFileListCoverThumbnails(['/book']);
		await flushMicrotasks();
		resetFileList();
		setCoverThumbnailLoader(load);
		scheduleFileListCoverThumbnails(['/book']);
		await flushMicrotasks();
		stale.resolve([1]);
		await flushMicrotasks();
		expect(getFileListCoverThumbnailState('/book').status).toBe('loading');
		fresh.resolve([2]);
		await flushMicrotasks();
		expect(getFileListCoverThumbnailState('/book')).toMatchObject({ status: 'ready' });
	});

	it('fetches fresh bytes after a ready thumbnail is removed and re-added', async () => {
		const load = vi.fn().mockResolvedValueOnce([1]).mockResolvedValueOnce([2]);
		setCoverThumbnailLoader(load);
		scheduleFileListCoverThumbnails(['/book']);
		await flushMicrotasks();
		removeFileListCoverThumbnail('/book');
		scheduleFileListCoverThumbnails(['/book']);
		await flushMicrotasks();
		expect(load).toHaveBeenCalledTimes(2);
		expect(getFileListCoverThumbnailState('/book')).toMatchObject({
			status: 'ready',
			dataUrl: 'data:image/jpeg;base64,Ag==',
		});
	});

	it('keeps the terminal thumbnail cache bounded even when every path is visible', async () => {
		const paths = Array.from(
			{ length: MAX_FILE_LIST_COVER_THUMBNAIL_CACHE_ENTRIES + 1 },
			(_, index) => `/book-${index}`,
		);
		setCoverThumbnailLoader(async () => [1]);
		scheduleFileListCoverThumbnails(paths);
		for (let index = 0; index < 150; index += 1) await Promise.resolve();
		expect(getFileListCoverThumbnailState(paths[0]).status).toBe('idle');
		expect(getFileListCoverThumbnailState(paths[paths.length - 1]).status).toBe('ready');
	});
});
