import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
	cancelMetadataLookupCoverPreviewSchedule,
	clearMetadataLookupCoverPreviewCache,
	getMetadataLookupCoverPreviewState,
	loadMetadataLookupCoverBytes,
	MAX_METADATA_LOOKUP_PREVIEW_CACHE_ENTRIES,
	scheduleMetadataLookupCoverPreviews,
} from '../metadataLookupCoverPreview.svelte';

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
	await Promise.resolve();
	await Promise.resolve();
	await Promise.resolve();
	await Promise.resolve();
	await Promise.resolve();
}

describe('metadata lookup cover preview scheduler', () => {
	beforeEach(() => {
		clearMetadataLookupCoverPreviewCache();
	});

	it('bounds eager preview concurrency', async () => {
		const requests: Array<Deferred<number[]>> = [];
		const loadCoverArtFromUrl = vi.fn((url: string) => {
			const request = createDeferred<number[]>();
			requests.push(request);
			return request.promise.then(() => [url.length]);
		});

		scheduleMetadataLookupCoverPreviews(
			['https://example.com/1.jpg', 'https://example.com/2.jpg', 'https://example.com/3.jpg'],
			loadCoverArtFromUrl,
		);
		await flushAsync();

		expect(loadCoverArtFromUrl).toHaveBeenCalledTimes(2);
		expect(getMetadataLookupCoverPreviewState('https://example.com/3.jpg').status).toBe(
			'queued',
		);

		requests[0].resolve([1]);
		await flushAsync();

		expect(loadCoverArtFromUrl).toHaveBeenCalledTimes(3);
		expect(loadCoverArtFromUrl).toHaveBeenLastCalledWith('https://example.com/3.jpg');

		requests[1].resolve([2]);
		requests[2].resolve([3]);
		await flushAsync();
	});

	it('dedupes duplicate cover URLs', async () => {
		const loadCoverArtFromUrl = vi.fn(async () => [1, 2, 3]);

		scheduleMetadataLookupCoverPreviews(
			['https://example.com/shared.jpg', 'https://example.com/shared.jpg'],
			loadCoverArtFromUrl,
		);
		await flushAsync();

		expect(loadCoverArtFromUrl).toHaveBeenCalledTimes(1);
	});

	it('ignores stale completions after a new schedule excludes the old URL', async () => {
		const first = createDeferred<number[]>();
		const second = createDeferred<number[]>();
		const loadCoverArtFromUrl = vi.fn((url: string) =>
			url.includes('first') ? first.promise : second.promise,
		);

		scheduleMetadataLookupCoverPreviews(['https://example.com/first.jpg'], loadCoverArtFromUrl);
		await flushAsync();
		cancelMetadataLookupCoverPreviewSchedule();
		scheduleMetadataLookupCoverPreviews(['https://example.com/second.jpg'], loadCoverArtFromUrl);
		await flushAsync();

		first.resolve([1]);
		await flushAsync();

		expect(getMetadataLookupCoverPreviewState('https://example.com/first.jpg').status).toBe(
			'idle',
		);

		second.resolve([2]);
		await flushAsync();

		expect(getMetadataLookupCoverPreviewState('https://example.com/second.jpg').status).toBe(
			'ready',
		);
	});

	it('keeps stale in-flight requests counted against the concurrency limit', async () => {
		const requests: Array<Deferred<number[]>> = [];
		const loadCoverArtFromUrl = vi.fn((url: string) => {
			const request = createDeferred<number[]>();
			requests.push(request);
			return request.promise.then(() => [url.length]);
		});

		scheduleMetadataLookupCoverPreviews(
			['https://example.com/stale-1.jpg', 'https://example.com/stale-2.jpg'],
			loadCoverArtFromUrl,
		);
		await flushAsync();
		cancelMetadataLookupCoverPreviewSchedule();
		scheduleMetadataLookupCoverPreviews(['https://example.com/fresh.jpg'], loadCoverArtFromUrl);
		await flushAsync();

		expect(loadCoverArtFromUrl).toHaveBeenCalledTimes(2);
		expect(getMetadataLookupCoverPreviewState('https://example.com/fresh.jpg').status).toBe(
			'queued',
		);

		requests[0].resolve([1]);
		await flushAsync();

		expect(loadCoverArtFromUrl).toHaveBeenCalledTimes(3);
		expect(loadCoverArtFromUrl).toHaveBeenLastCalledWith('https://example.com/fresh.jpg');

		requests[1].resolve([2]);
		requests[2].resolve([3]);
		await flushAsync();
	});

	it('commits an in-flight preview when the same URL remains visible after reschedule', async () => {
		const request = createDeferred<number[]>();
		const loadCoverArtFromUrl = vi.fn(() => request.promise);
		const coverUrl = 'https://example.com/same-visible.jpg';

		scheduleMetadataLookupCoverPreviews([coverUrl], loadCoverArtFromUrl);
		await flushAsync();
		cancelMetadataLookupCoverPreviewSchedule();
		scheduleMetadataLookupCoverPreviews([coverUrl], loadCoverArtFromUrl);
		await flushAsync();

		request.resolve([7, 7, 7]);
		await flushAsync();

		expect(loadCoverArtFromUrl).toHaveBeenCalledTimes(1);
		expect(getMetadataLookupCoverPreviewState(coverUrl).status).toBe('ready');
	});

	it('does not repopulate cache after clear while request is in flight', async () => {
		const request = createDeferred<number[]>();
		const loadCoverArtFromUrl = vi.fn(() => request.promise);

		scheduleMetadataLookupCoverPreviews(['https://example.com/clear.jpg'], loadCoverArtFromUrl);
		await flushAsync();
		clearMetadataLookupCoverPreviewCache();

		request.resolve([9]);
		await flushAsync();

		expect(getMetadataLookupCoverPreviewState('https://example.com/clear.jpg').status).toBe(
			'idle',
		);
	});

	it('caps cached previews by pruning the oldest ready entries', async () => {
		const loadCoverArtFromUrl = vi.fn(async (url: string) => [url.length]);
		const urls = Array.from(
			{ length: MAX_METADATA_LOOKUP_PREVIEW_CACHE_ENTRIES + 1 },
			(_, index) => `https://example.com/cache-${index}.jpg`,
		);

		for (const url of urls) {
			await loadMetadataLookupCoverBytes(url, loadCoverArtFromUrl);
		}
		await flushAsync();

		expect(getMetadataLookupCoverPreviewState(urls[0]).status).toBe('idle');
		expect(getMetadataLookupCoverPreviewState(urls[urls.length - 1]).status).toBe('ready');
	});

	it('lets apply join an in-flight preview request', async () => {
		const request = createDeferred<number[]>();
		const loadCoverArtFromUrl = vi.fn(() => request.promise);

		scheduleMetadataLookupCoverPreviews(['https://example.com/apply.jpg'], loadCoverArtFromUrl);
		await flushAsync();
		const bytesPromise = loadMetadataLookupCoverBytes(
			'https://example.com/apply.jpg',
			loadCoverArtFromUrl,
		);

		request.resolve([4, 5, 6]);
		await expect(bytesPromise).resolves.toEqual([4, 5, 6]);
		expect(loadCoverArtFromUrl).toHaveBeenCalledTimes(1);
	});
});
