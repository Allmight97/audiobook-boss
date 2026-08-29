import { describe, expect, it, vi } from 'vitest';
import type { BeforeUnloadTarget } from '../services/progressSubscription';
import { createProgressSubscription } from '../services/progressSubscription';

function createDeferred<T>() {
	let resolve!: (value: T) => void;
	let reject!: (reason?: unknown) => void;
	const promise = new Promise<T>((resolvePromise, rejectPromise) => {
		resolve = resolvePromise;
		reject = rejectPromise;
	});

	return {
		promise,
		resolve,
		reject,
	};
}

function createBeforeUnloadTargetMock() {
	let beforeUnloadHandler: (() => void) | undefined;
	const addEventListener = vi.fn((type: 'beforeunload', listener: () => void) => {
		if (type === 'beforeunload') {
			beforeUnloadHandler = listener;
		}
	});
	const removeEventListener = vi.fn((type: 'beforeunload', listener: () => void) => {
		if (type === 'beforeunload' && beforeUnloadHandler === listener) {
			beforeUnloadHandler = undefined;
		}
	});

	return {
		target: {
			addEventListener,
			removeEventListener,
		} satisfies BeforeUnloadTarget,
		addEventListener,
		removeEventListener,
		fireBeforeUnload: () => {
			beforeUnloadHandler?.();
		},
		hasBeforeUnloadHandler: () => Boolean(beforeUnloadHandler),
	};
}

describe('progress subscription owner', () => {
	it('starts once and tears down once across repeated calls', async () => {
		const eventTarget = createBeforeUnloadTargetMock();
		const progressUnlisten = vi.fn();
		const queueUnlisten = vi.fn();
		const listenForProgressEvents = vi.fn().mockResolvedValue(progressUnlisten);
		const listenForQueueEvents = vi.fn().mockResolvedValue(queueUnlisten);
		const owner = createProgressSubscription({
			onProgress: vi.fn(),
			onQueue: vi.fn(),
			listenForProgressEvents,
			listenForQueueEvents,
			eventTarget: eventTarget.target,
		});

		await owner.start();
		await owner.start();

		expect(listenForProgressEvents).toHaveBeenCalledTimes(1);
		expect(listenForQueueEvents).toHaveBeenCalledTimes(1);
		expect(eventTarget.addEventListener).toHaveBeenCalledTimes(1);
		expect(eventTarget.hasBeforeUnloadHandler()).toBe(true);

		owner.stop();
		owner.stop();

		expect(progressUnlisten).toHaveBeenCalledTimes(1);
		expect(queueUnlisten).toHaveBeenCalledTimes(1);
		expect(eventTarget.removeEventListener).toHaveBeenCalledTimes(1);
		expect(eventTarget.hasBeforeUnloadHandler()).toBe(false);
	});

	it('tears down active listeners on beforeunload', async () => {
		const eventTarget = createBeforeUnloadTargetMock();
		const progressUnlisten = vi.fn();
		const queueUnlisten = vi.fn();
		const owner = createProgressSubscription({
			onProgress: vi.fn(),
			onQueue: vi.fn(),
			listenForProgressEvents: vi.fn().mockResolvedValue(progressUnlisten),
			listenForQueueEvents: vi.fn().mockResolvedValue(queueUnlisten),
			eventTarget: eventTarget.target,
		});

		await owner.start();
		eventTarget.fireBeforeUnload();

		expect(progressUnlisten).toHaveBeenCalledTimes(1);
		expect(queueUnlisten).toHaveBeenCalledTimes(1);
		expect(eventTarget.removeEventListener).toHaveBeenCalledTimes(1);
		expect(eventTarget.hasBeforeUnloadHandler()).toBe(false);
	});

	it('cleans up listeners that resolve after stop during startup', async () => {
		const eventTarget = createBeforeUnloadTargetMock();
		const progressDeferred = createDeferred<() => void>();
		const queueDeferred = createDeferred<() => void>();
		const progressUnlisten = vi.fn();
		const queueUnlisten = vi.fn();
		const listenForProgressEvents = vi.fn(() => progressDeferred.promise);
		const listenForQueueEvents = vi.fn(() => queueDeferred.promise);
		const owner = createProgressSubscription({
			onProgress: vi.fn(),
			onQueue: vi.fn(),
			listenForProgressEvents,
			listenForQueueEvents,
			eventTarget: eventTarget.target,
		});

		const startPromise = owner.start();
		progressDeferred.resolve(progressUnlisten);
		await vi.waitFor(() => expect(listenForQueueEvents).toHaveBeenCalledTimes(1));

		owner.stop();
		queueDeferred.resolve(queueUnlisten);
		await startPromise;

		expect(progressUnlisten).toHaveBeenCalledTimes(1);
		expect(queueUnlisten).toHaveBeenCalledTimes(1);
		expect(eventTarget.removeEventListener).toHaveBeenCalledTimes(1);
		expect(eventTarget.hasBeforeUnloadHandler()).toBe(false);
	});

	it('cleans up failed startup and allows retry', async () => {
		const eventTarget = createBeforeUnloadTargetMock();
		const firstProgressUnlisten = vi.fn();
		const secondProgressUnlisten = vi.fn();
		const secondQueueUnlisten = vi.fn();
		const listenForProgressEvents = vi
			.fn()
			.mockResolvedValueOnce(firstProgressUnlisten)
			.mockResolvedValueOnce(secondProgressUnlisten);
		const listenForQueueEvents = vi
			.fn()
			.mockRejectedValueOnce(new Error('queue subscription failed'))
			.mockResolvedValueOnce(secondQueueUnlisten);
		const owner = createProgressSubscription({
			onProgress: vi.fn(),
			onQueue: vi.fn(),
			listenForProgressEvents,
			listenForQueueEvents,
			eventTarget: eventTarget.target,
		});

		await expect(owner.start()).rejects.toThrow('queue subscription failed');
		expect(firstProgressUnlisten).toHaveBeenCalledTimes(1);
		expect(eventTarget.removeEventListener).toHaveBeenCalledTimes(1);

		await owner.start();
		expect(listenForProgressEvents).toHaveBeenCalledTimes(2);
		expect(listenForQueueEvents).toHaveBeenCalledTimes(2);

		owner.dispose();
		expect(secondProgressUnlisten).toHaveBeenCalledTimes(1);
		expect(secondQueueUnlisten).toHaveBeenCalledTimes(1);
		expect(eventTarget.removeEventListener).toHaveBeenCalledTimes(2);
		await expect(owner.start()).rejects.toThrow('Progress subscription has been disposed.');
	});
});
