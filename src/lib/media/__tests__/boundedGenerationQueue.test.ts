import { describe, expect, it } from 'vitest';
import { createBoundedGenerationQueue } from '../boundedGenerationQueue';

type Deferred = { promise: Promise<void>; resolve: () => void };
function createDeferred(): Deferred {
	let resolve!: () => void;
	const promise = new Promise<void>((resolvePromise) => {
		resolve = resolvePromise;
	});
	return { promise, resolve };
}
async function flushAsync(): Promise<void> {
	for (let index = 0; index < 12; index += 1) await Promise.resolve();
}

describe('bounded generation queue', () => {
	it('keeps stale active work counted before starting replacement work', async () => {
		const queue = createBoundedGenerationQueue(2);
		const tasks = new Map<string, Deferred>();
		const started: string[] = [];
		const schedule = (keys: string[]) =>
			queue.schedule(keys, {
				prepare: () => true,
				start: (key) => {
					started.push(key);
					const task = createDeferred();
					tasks.set(key, task);
					return task.promise;
				},
			});
		schedule(['stale-a', 'stale-b']);
		queue.cancel();
		schedule(['fresh']);
		await flushAsync();
		expect(started).toEqual(['stale-a', 'stale-b']);
		tasks.get('stale-a')?.resolve();
		await flushAsync();
		expect(started).toEqual(['stale-a', 'stale-b', 'fresh']);
		tasks.get('stale-b')?.resolve();
		tasks.get('fresh')?.resolve();
		await flushAsync();
	});
});
