import { describe, expect, it, vi } from 'vitest';
import { createSubscriptionGroup } from './subscriptionGroup';

function createDeferred<T>() {
	let resolve!: (value: T) => void;
	const promise = new Promise<T>((resolvePromise) => {
		resolve = resolvePromise;
	});
	return { promise, resolve };
}

describe('subscription group', () => {
	it('invokes every registered unlisten exactly once on dispose', async () => {
		const group = createSubscriptionGroup();
		const fromPromise = vi.fn();
		const fromSync = vi.fn();
		await group.add(Promise.resolve(fromPromise));
		await group.add(fromSync);
		expect(group.disposed).toBe(false);

		group.dispose();
		group.dispose(); // idempotent

		expect(fromPromise).toHaveBeenCalledTimes(1);
		expect(fromSync).toHaveBeenCalledTimes(1);
		expect(group.disposed).toBe(true);
	});

	it('immediately unlistens a registration that resolves after dispose', async () => {
		const group = createSubscriptionGroup();
		const deferred = createDeferred<() => void>();
		const unlisten = vi.fn();

		const pending = group.add(deferred.promise);
		group.dispose();
		deferred.resolve(unlisten);
		await pending;

		expect(unlisten).toHaveBeenCalledTimes(1);
	});

	it('immediately unlistens a sync registration added after dispose', async () => {
		const group = createSubscriptionGroup();
		const unlisten = vi.fn();
		group.dispose();
		await group.add(unlisten);
		expect(unlisten).toHaveBeenCalledTimes(1);
	});
});
