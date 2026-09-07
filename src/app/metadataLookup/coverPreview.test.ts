import { describe, expect, it, vi } from 'vitest';
import {
	createMetadataLookupCoverPreviews,
	MAX_METADATA_LOOKUP_PREVIEW_CACHE_ENTRIES,
	type MetadataLookupCoverPreviews,
} from './coverPreview';

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

function makePreviews(
	loadCoverArtFromUrl: (url: string) => Promise<number[]>,
): MetadataLookupCoverPreviews {
	return createMetadataLookupCoverPreviews({
		loadCoverArtFromUrl,
		onChange: () => undefined,
	});
}

describe('metadata lookup cover preview scheduler', () => {
	it('bounds eager preview concurrency', async () => {
		const requests: Array<Deferred<number[]>> = [];
		const loadCoverArtFromUrl = vi.fn((url: string) => {
			const request = createDeferred<number[]>();
			requests.push(request);
			return request.promise.then(() => [url.length]);
		});
		const previews = makePreviews(loadCoverArtFromUrl);

		previews.schedule([
			'https://example.com/1.jpg',
			'https://example.com/2.jpg',
			'https://example.com/3.jpg',
		]);
		await flushAsync();

		expect(loadCoverArtFromUrl).toHaveBeenCalledTimes(2);
		expect(previews.getState('https://example.com/3.jpg').status).toBe('queued');

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
		const previews = makePreviews(loadCoverArtFromUrl);

		previews.schedule(['https://example.com/shared.jpg', 'https://example.com/shared.jpg']);
		await flushAsync();

		expect(loadCoverArtFromUrl).toHaveBeenCalledTimes(1);
	});

	it('ignores stale completions after a new schedule excludes the old URL', async () => {
		const first = createDeferred<number[]>();
		const second = createDeferred<number[]>();
		const loadCoverArtFromUrl = vi.fn((url: string) =>
			url.includes('first') ? first.promise : second.promise,
		);
		const previews = makePreviews(loadCoverArtFromUrl);

		previews.schedule(['https://example.com/first.jpg']);
		await flushAsync();
		previews.cancel();
		previews.schedule(['https://example.com/second.jpg']);
		await flushAsync();

		first.resolve([1]);
		await flushAsync();

		expect(previews.getState('https://example.com/first.jpg').status).toBe('idle');

		second.resolve([2]);
		await flushAsync();

		expect(previews.getState('https://example.com/second.jpg').status).toBe('ready');
	});

	it('keeps stale in-flight requests counted against the concurrency limit', async () => {
		const requests: Array<Deferred<number[]>> = [];
		const loadCoverArtFromUrl = vi.fn((url: string) => {
			const request = createDeferred<number[]>();
			requests.push(request);
			return request.promise.then(() => [url.length]);
		});
		const previews = makePreviews(loadCoverArtFromUrl);

		previews.schedule(['https://example.com/stale-1.jpg', 'https://example.com/stale-2.jpg']);
		await flushAsync();
		previews.cancel();
		previews.schedule(['https://example.com/fresh.jpg']);
		await flushAsync();

		expect(loadCoverArtFromUrl).toHaveBeenCalledTimes(2);
		expect(previews.getState('https://example.com/fresh.jpg').status).toBe('queued');

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
		const previews = makePreviews(loadCoverArtFromUrl);

		previews.schedule([coverUrl]);
		await flushAsync();
		previews.cancel();
		previews.schedule([coverUrl]);
		await flushAsync();

		request.resolve([7, 7, 7]);
		await flushAsync();

		expect(loadCoverArtFromUrl).toHaveBeenCalledTimes(1);
		expect(previews.getState(coverUrl).status).toBe('ready');
	});

	it('does not repopulate cache after clear while request is in flight', async () => {
		const request = createDeferred<number[]>();
		const loadCoverArtFromUrl = vi.fn(() => request.promise);
		const previews = makePreviews(loadCoverArtFromUrl);

		previews.schedule(['https://example.com/clear.jpg']);
		await flushAsync();
		previews.clear();

		request.resolve([9]);
		await flushAsync();

		expect(previews.getState('https://example.com/clear.jpg').status).toBe('idle');
	});

	it('caps cached previews by pruning the oldest ready entries', async () => {
		const loadCoverArtFromUrl = vi.fn(async (url: string) => [url.length]);
		const previews = makePreviews(loadCoverArtFromUrl);
		const urls = Array.from(
			{ length: MAX_METADATA_LOOKUP_PREVIEW_CACHE_ENTRIES + 1 },
			(_, index) => `https://example.com/cache-${index}.jpg`,
		);

		for (const url of urls) {
			await previews.loadBytes(url);
		}
		await flushAsync();

		expect(previews.getState(urls[0]).status).toBe('idle');
		expect(previews.getState(urls[urls.length - 1]).status).toBe('ready');
	});

	it('keeps scheduled visible previews ready when the visible list exceeds the cache cap', async () => {
		const loadCoverArtFromUrl = vi.fn(async (url: string) => [url.length]);
		const previews = makePreviews(loadCoverArtFromUrl);
		const urls = Array.from(
			{ length: MAX_METADATA_LOOKUP_PREVIEW_CACHE_ENTRIES + 1 },
			(_, index) => `https://example.com/visible-${index}.jpg`,
		);

		previews.schedule(urls);
		for (let index = 0; index < urls.length; index += 1) {
			await flushAsync();
		}

		expect(loadCoverArtFromUrl).toHaveBeenCalledTimes(urls.length);
		expect(previews.getState(urls[0]).status).toBe('ready');
		expect(previews.getState(urls[urls.length - 1]).status).toBe('ready');
	});

	it('lets apply join an in-flight preview request', async () => {
		const request = createDeferred<number[]>();
		const loadCoverArtFromUrl = vi.fn(() => request.promise);
		const previews = makePreviews(loadCoverArtFromUrl);

		previews.schedule(['https://example.com/apply.jpg']);
		await flushAsync();
		const bytesPromise = previews.loadBytes('https://example.com/apply.jpg');

		request.resolve([4, 5, 6]);
		await expect(bytesPromise).resolves.toEqual([4, 5, 6]);
		expect(loadCoverArtFromUrl).toHaveBeenCalledTimes(1);
	});

	it('keeps cancellation and cache state isolated across preview instances', async () => {
		const firstLoad = createDeferred<number[]>();
		const first = makePreviews(() => firstLoad.promise);
		const second = makePreviews(async () => [0xff, 0xd8, 0xff]);

		first.schedule(['https://covers.example/first.jpg']);
		second.schedule(['https://covers.example/second.jpg']);
		await flushAsync();

		expect(second.getState('https://covers.example/second.jpg').status).toBe('ready');

		first.clear();
		firstLoad.resolve([0xff, 0xd8, 0xff]);
		await flushAsync();

		expect(first.getState('https://covers.example/first.jpg').status).toBe('idle');
		expect(second.getState('https://covers.example/second.jpg').status).toBe('ready');
	});
});
