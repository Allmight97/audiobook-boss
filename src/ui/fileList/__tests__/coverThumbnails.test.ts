import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
	clearFileListCoverThumbnails,
	getFileListCoverThumbnailState,
	removeFileListCoverThumbnail,
	scheduleFileListCoverThumbnails,
} from '../coverThumbnails.svelte';

type Deferred<T> = {
	promise: Promise<T>;
	resolve: (value: T) => void;
	reject: (reason?: unknown) => void;
};

function createDeferred<T>(): Deferred<T> {
	let resolve!: (value: T) => void;
	let reject!: (reason?: unknown) => void;
	const promise = new Promise<T>((resolvePromise, rejectPromise) => {
		resolve = resolvePromise;
		reject = rejectPromise;
	});
	return { promise, resolve, reject };
}

async function flushAsync(): Promise<void> {
	for (let index = 0; index < 12; index += 1) {
		await Promise.resolve();
	}
}

describe('FileList cover thumbnail scheduler', () => {
	beforeEach(() => {
		clearFileListCoverThumbnails();
	});

	it('bounds loading to two paths and dedupes duplicate paths', async () => {
		const requests: Array<Deferred<number[] | null>> = [];
		const loadThumbnail = vi.fn(() => {
			const request = createDeferred<number[] | null>();
			requests.push(request);
			return request.promise;
		});

		scheduleFileListCoverThumbnails(
			['/books/a.m4b', '/books/a.m4b', '/books/b.m4b', '/books/c.m4b'],
			loadThumbnail,
		);
		await flushAsync();

		expect(loadThumbnail).toHaveBeenCalledTimes(2);
		expect(getFileListCoverThumbnailState('/books/c.m4b').status).toBe('queued');

		requests[0].resolve([1]);
		await flushAsync();
		expect(loadThumbnail).toHaveBeenCalledTimes(3);
		expect(loadThumbnail).toHaveBeenLastCalledWith('/books/c.m4b');

		requests[1].resolve([2]);
		requests[2].resolve([3]);
		await flushAsync();
	});

	it('schedules only appended paths when the prior paths are settled', async () => {
		const loadThumbnail = vi.fn(async (path: string) => [path.length]);

		scheduleFileListCoverThumbnails(['/books/a.m4b'], loadThumbnail);
		await flushAsync();
		scheduleFileListCoverThumbnails(['/books/a.m4b', '/books/b.m4b'], loadThumbnail);
		await flushAsync();

		expect(loadThumbnail).toHaveBeenCalledTimes(2);
		expect(loadThumbnail).toHaveBeenLastCalledWith('/books/b.m4b');
	});

	it('ignores stale completions after replacement or clear', async () => {
		const oldRequest = createDeferred<number[] | null>();
		const replacementRequest = createDeferred<number[] | null>();
		const loadThumbnail = vi.fn((path: string) =>
			path.includes('old') ? oldRequest.promise : replacementRequest.promise,
		);

		scheduleFileListCoverThumbnails(['/books/old.m4b'], loadThumbnail);
		await flushAsync();
		scheduleFileListCoverThumbnails(['/books/new.m4b'], loadThumbnail);
		await flushAsync();
		oldRequest.resolve([1]);
		await flushAsync();

		expect(getFileListCoverThumbnailState('/books/old.m4b').status).toBe('idle');
		expect(getFileListCoverThumbnailState('/books/new.m4b').status).toBe('loading');

		replacementRequest.resolve([2]);
		await flushAsync();
		expect(getFileListCoverThumbnailState('/books/new.m4b').status).toBe('ready');

		const clearRequest = createDeferred<number[] | null>();
		scheduleFileListCoverThumbnails(['/books/clear.m4b'], () => clearRequest.promise);
		await flushAsync();
		clearFileListCoverThumbnails();
		clearRequest.resolve([3]);
		await flushAsync();

		expect(getFileListCoverThumbnailState('/books/clear.m4b').status).toBe('idle');
	});

	it('keeps absent and failed paths terminal instead of retrying on reschedule', async () => {
		const absentLoader = vi.fn(async () => null);
		scheduleFileListCoverThumbnails(['/books/no-cover.m4b'], absentLoader);
		await flushAsync();
		scheduleFileListCoverThumbnails(['/books/no-cover.m4b'], absentLoader);
		await flushAsync();

		expect(getFileListCoverThumbnailState('/books/no-cover.m4b').status).toBe('absent');
		expect(absentLoader).toHaveBeenCalledTimes(1);

		const errorLoader = vi.fn(async () => {
			throw new Error('unreadable');
		});
		const warn = vi.spyOn(console, 'warn').mockImplementation(() => undefined);
		scheduleFileListCoverThumbnails(['/books/error.m4b'], errorLoader);
		await flushAsync();
		scheduleFileListCoverThumbnails(['/books/error.m4b'], errorLoader);
		await flushAsync();
		warn.mockRestore();

		expect(getFileListCoverThumbnailState('/books/error.m4b').status).toBe('error');
		expect(errorLoader).toHaveBeenCalledTimes(1);
	});

	it('fetches fresh bytes after a ready thumbnail is removed and re-added', async () => {
		const path = '/books/readded.m4b';
		const loadThumbnail = vi.fn()
			.mockResolvedValueOnce([1])
			.mockResolvedValueOnce([2]);

		scheduleFileListCoverThumbnails([path], loadThumbnail);
		await flushAsync();
		const staleDataUrl = getFileListCoverThumbnailState(path);
		expect(staleDataUrl.status).toBe('ready');

		removeFileListCoverThumbnail(path);
		scheduleFileListCoverThumbnails([path], loadThumbnail);
		await flushAsync();

		expect(loadThumbnail).toHaveBeenCalledTimes(2);
		expect(getFileListCoverThumbnailState(path)).toEqual({
			status: 'ready',
			dataUrl: 'data:image/jpeg;base64,Ag==',
		});
	});

	it('does not commit pre-removal bytes when an in-flight thumbnail path is re-added', async () => {
		const path = '/books/inflight-readded.m4b';
		const staleRequest = createDeferred<number[] | null>();
		const freshRequest = createDeferred<number[] | null>();
		const loadThumbnail = vi.fn()
			.mockReturnValueOnce(staleRequest.promise)
			.mockReturnValueOnce(freshRequest.promise);

		scheduleFileListCoverThumbnails([path], loadThumbnail);
		await flushAsync();
		removeFileListCoverThumbnail(path);
		scheduleFileListCoverThumbnails([path], loadThumbnail);
		await flushAsync();

		expect(loadThumbnail).toHaveBeenCalledTimes(2);
		staleRequest.resolve([1]);
		await flushAsync();
		expect(getFileListCoverThumbnailState(path).status).toBe('loading');

		freshRequest.resolve([2]);
		await flushAsync();
		expect(getFileListCoverThumbnailState(path)).toEqual({
			status: 'ready',
			dataUrl: 'data:image/jpeg;base64,Ag==',
		});
	});
});
