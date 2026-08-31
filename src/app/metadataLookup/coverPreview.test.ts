import { beforeEach, describe, expect, it, vi } from 'vitest';
import { JPEG_DATA_URL } from '../../test/fixtures/coverCapability';
import {
	cancelMetadataLookupCoverPreviewSchedule,
	clearMetadataLookupCoverPreviewCache,
	fetchMetadataLookupCoverPreview,
	getMetadataLookupCoverPreviewState,
	MAX_METADATA_LOOKUP_PREVIEW_CACHE_ENTRIES,
	scheduleMetadataLookupCoverPreviews,
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

function previewDataUrl(url: string): string {
	return `${JPEG_DATA_URL}${url.length}`;
}

describe('metadata lookup cover preview scheduler', () => {
	beforeEach(() => {
		clearMetadataLookupCoverPreviewCache();
	});

	it('bounds eager preview concurrency', async () => {
		const requests: Array<Deferred<string>> = [];
		const loadCoverPreview = vi.fn((url: string) => {
			const request = createDeferred<string>();
			requests.push(request);
			return request.promise.then(() => previewDataUrl(url));
		});

		scheduleMetadataLookupCoverPreviews(
			['https://example.com/1.jpg', 'https://example.com/2.jpg', 'https://example.com/3.jpg'],
			loadCoverPreview,
		);
		await flushAsync();

		expect(loadCoverPreview).toHaveBeenCalledTimes(2);
		expect(getMetadataLookupCoverPreviewState('https://example.com/3.jpg').status).toBe('queued');

		requests[0].resolve(previewDataUrl('https://example.com/1.jpg'));
		await flushAsync();

		expect(loadCoverPreview).toHaveBeenCalledTimes(3);
		expect(loadCoverPreview).toHaveBeenLastCalledWith('https://example.com/3.jpg');

		requests[1].resolve(previewDataUrl('https://example.com/2.jpg'));
		requests[2].resolve(previewDataUrl('https://example.com/3.jpg'));
		await flushAsync();
	});

	it('dedupes duplicate cover URLs', async () => {
		const loadCoverPreview = vi.fn(async () => JPEG_DATA_URL);

		scheduleMetadataLookupCoverPreviews(
			['https://example.com/shared.jpg', 'https://example.com/shared.jpg'],
			loadCoverPreview,
		);
		await flushAsync();

		expect(loadCoverPreview).toHaveBeenCalledTimes(1);
	});

	it('ignores stale completions after a new schedule excludes the old URL', async () => {
		const first = createDeferred<string>();
		const second = createDeferred<string>();
		const loadCoverPreview = vi.fn((url: string) =>
			url.includes('first') ? first.promise : second.promise,
		);

		scheduleMetadataLookupCoverPreviews(['https://example.com/first.jpg'], loadCoverPreview);
		await flushAsync();
		cancelMetadataLookupCoverPreviewSchedule();
		scheduleMetadataLookupCoverPreviews(['https://example.com/second.jpg'], loadCoverPreview);
		await flushAsync();

		first.resolve(JPEG_DATA_URL);
		await flushAsync();

		expect(getMetadataLookupCoverPreviewState('https://example.com/first.jpg').status).toBe('idle');

		second.resolve(JPEG_DATA_URL);
		await flushAsync();

		expect(getMetadataLookupCoverPreviewState('https://example.com/second.jpg').status).toBe(
			'ready',
		);
	});

	it('keeps stale in-flight requests counted against the concurrency limit', async () => {
		const requests: Array<Deferred<string>> = [];
		const loadCoverPreview = vi.fn((url: string) => {
			const request = createDeferred<string>();
			requests.push(request);
			return request.promise.then(() => previewDataUrl(url));
		});

		scheduleMetadataLookupCoverPreviews(
			['https://example.com/stale-1.jpg', 'https://example.com/stale-2.jpg'],
			loadCoverPreview,
		);
		await flushAsync();
		cancelMetadataLookupCoverPreviewSchedule();
		scheduleMetadataLookupCoverPreviews(['https://example.com/fresh.jpg'], loadCoverPreview);
		await flushAsync();

		expect(loadCoverPreview).toHaveBeenCalledTimes(2);
		expect(getMetadataLookupCoverPreviewState('https://example.com/fresh.jpg').status).toBe(
			'queued',
		);

		requests[0].resolve(previewDataUrl('https://example.com/stale-1.jpg'));
		await flushAsync();

		expect(loadCoverPreview).toHaveBeenCalledTimes(3);
		expect(loadCoverPreview).toHaveBeenLastCalledWith('https://example.com/fresh.jpg');

		requests[1].resolve(previewDataUrl('https://example.com/stale-2.jpg'));
		requests[2].resolve(previewDataUrl('https://example.com/fresh.jpg'));
		await flushAsync();
	});

	it('commits an in-flight preview when the same URL remains visible after reschedule', async () => {
		const request = createDeferred<string>();
		const loadCoverPreview = vi.fn(() => request.promise);
		const coverUrl = 'https://example.com/same-visible.jpg';

		scheduleMetadataLookupCoverPreviews([coverUrl], loadCoverPreview);
		await flushAsync();
		cancelMetadataLookupCoverPreviewSchedule();
		scheduleMetadataLookupCoverPreviews([coverUrl], loadCoverPreview);
		await flushAsync();

		request.resolve(JPEG_DATA_URL);
		await flushAsync();

		expect(loadCoverPreview).toHaveBeenCalledTimes(1);
		expect(getMetadataLookupCoverPreviewState(coverUrl).status).toBe('ready');
	});

	it('does not repopulate cache after clear while request is in flight', async () => {
		const request = createDeferred<string>();
		const loadCoverPreview = vi.fn(() => request.promise);

		scheduleMetadataLookupCoverPreviews(['https://example.com/clear.jpg'], loadCoverPreview);
		await flushAsync();
		clearMetadataLookupCoverPreviewCache();

		request.resolve(JPEG_DATA_URL);
		await flushAsync();

		expect(getMetadataLookupCoverPreviewState('https://example.com/clear.jpg').status).toBe('idle');
	});

	it('caps cached previews by pruning the oldest ready entries', async () => {
		const loadCoverPreview = vi.fn(async (url: string) => previewDataUrl(url));
		const urls = Array.from(
			{ length: MAX_METADATA_LOOKUP_PREVIEW_CACHE_ENTRIES + 1 },
			(_, index) => `https://example.com/cache-${index}.jpg`,
		);

		for (const url of urls) {
			await fetchMetadataLookupCoverPreview(url, loadCoverPreview);
		}
		await flushAsync();

		expect(getMetadataLookupCoverPreviewState(urls[0]).status).toBe('idle');
		expect(getMetadataLookupCoverPreviewState(urls[urls.length - 1]).status).toBe('ready');
	});

	it('keeps scheduled visible previews ready when the visible list exceeds the cache cap', async () => {
		const loadCoverPreview = vi.fn(async (url: string) => previewDataUrl(url));
		const urls = Array.from(
			{ length: MAX_METADATA_LOOKUP_PREVIEW_CACHE_ENTRIES + 1 },
			(_, index) => `https://example.com/visible-${index}.jpg`,
		);

		scheduleMetadataLookupCoverPreviews(urls, loadCoverPreview);
		for (let index = 0; index < urls.length; index += 1) {
			await flushAsync();
		}

		expect(loadCoverPreview).toHaveBeenCalledTimes(urls.length);
		expect(getMetadataLookupCoverPreviewState(urls[0]).status).toBe('ready');
		expect(getMetadataLookupCoverPreviewState(urls[urls.length - 1]).status).toBe('ready');
	});

	it('lets a later fetch join an in-flight display preview', async () => {
		const request = createDeferred<string>();
		const loadCoverPreview = vi.fn(() => request.promise);

		scheduleMetadataLookupCoverPreviews(['https://example.com/apply.jpg'], loadCoverPreview);
		await flushAsync();
		const fetchPromise = fetchMetadataLookupCoverPreview(
			'https://example.com/apply.jpg',
			loadCoverPreview,
		);

		request.resolve(JPEG_DATA_URL);
		await fetchPromise;
		expect(loadCoverPreview).toHaveBeenCalledTimes(1);
		expect(getMetadataLookupCoverPreviewState('https://example.com/apply.jpg')).toEqual({
			status: 'ready',
			dataUrl: JPEG_DATA_URL,
		});
	});
});
