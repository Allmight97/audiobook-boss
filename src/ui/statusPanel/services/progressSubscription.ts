import { listenForProgressEvents, listenForQueueEvents } from '../events';
import type { ProcessingProgressEvent, ProcessingQueueEvent } from '../../../types/events';

type Unlisten = () => void;
type ProgressListenerRegistration = (
	onProgress: (event: ProcessingProgressEvent) => void,
) => Promise<Unlisten>;
type QueueListenerRegistration = (
	onQueue: (event: ProcessingQueueEvent) => void,
) => Promise<Unlisten>;

export interface BeforeUnloadTarget {
	addEventListener(type: 'beforeunload', listener: () => void): void;
	removeEventListener(type: 'beforeunload', listener: () => void): void;
}

export interface ProgressSubscriptionOwner {
	start(): Promise<void>;
	stop(): void;
	dispose(): void;
}

interface CreateProgressSubscriptionOptions {
	onProgress: (event: ProcessingProgressEvent) => void;
	onQueue: (event: ProcessingQueueEvent) => void;
	listenForProgressEvents?: ProgressListenerRegistration;
	listenForQueueEvents?: QueueListenerRegistration;
	eventTarget?: BeforeUnloadTarget;
}

function getDefaultEventTarget(): BeforeUnloadTarget | undefined {
	return typeof window === 'undefined' ? undefined : window;
}

export function createProgressSubscription({
	onProgress,
	onQueue,
	listenForProgressEvents: subscribeToProgress = listenForProgressEvents,
	listenForQueueEvents: subscribeToQueue = listenForQueueEvents,
	eventTarget = getDefaultEventTarget(),
}: CreateProgressSubscriptionOptions): ProgressSubscriptionOwner {
	let progressUnlisten: Unlisten | undefined;
	let queueUnlisten: Unlisten | undefined;
	let beforeUnloadAttached = false;
	let disposed = false;
	let subscriptionToken = 0;
	let startPromise: Promise<void> | null = null;

	const detachActiveListeners = () => {
		const progressListener = progressUnlisten;
		const queueListener = queueUnlisten;
		progressUnlisten = undefined;
		queueUnlisten = undefined;
		progressListener?.();
		queueListener?.();
	};

	const attachBeforeUnload = () => {
		if (!eventTarget || beforeUnloadAttached) {
			return;
		}

		eventTarget.addEventListener('beforeunload', handleBeforeUnload);
		beforeUnloadAttached = true;
	};

	const detachBeforeUnload = () => {
		if (!eventTarget || !beforeUnloadAttached) {
			return;
		}

		eventTarget.removeEventListener('beforeunload', handleBeforeUnload);
		beforeUnloadAttached = false;
	};

	const teardown = () => {
		subscriptionToken += 1;
		startPromise = null;
		detachActiveListeners();
		detachBeforeUnload();
	};

	const handleBeforeUnload = () => {
		teardown();
	};

	const ensureSubscribed = async () => {
		if (disposed) {
			throw new Error('Progress subscription has been disposed.');
		}

		if (progressUnlisten && queueUnlisten) {
			return;
		}

		if (startPromise) {
			return startPromise;
		}

		const token = ++subscriptionToken;
		attachBeforeUnload();

		let pendingStart: Promise<void> | null = null;
		pendingStart = (async () => {
			let nextProgressUnlisten: Unlisten | undefined;
			let nextQueueUnlisten: Unlisten | undefined;

			try {
				nextProgressUnlisten = await subscribeToProgress(onProgress);
				if (token !== subscriptionToken || disposed) {
					nextProgressUnlisten();
					return;
				}

				nextQueueUnlisten = await subscribeToQueue(onQueue);
				if (token !== subscriptionToken || disposed) {
					nextProgressUnlisten();
					nextQueueUnlisten();
					return;
				}

				progressUnlisten = nextProgressUnlisten;
				queueUnlisten = nextQueueUnlisten;
			} catch (error) {
				nextProgressUnlisten?.();
				nextQueueUnlisten?.();
				if (token === subscriptionToken) {
					detachBeforeUnload();
				}
				throw error;
			} finally {
				if (startPromise === pendingStart) {
					startPromise = null;
				}
			}
		})();

		startPromise = pendingStart;
		return pendingStart;
	};

	const dispose = () => {
		if (disposed) {
			return;
		}

		disposed = true;
		teardown();
	};

	return {
		start: ensureSubscribed,
		stop: teardown,
		dispose,
	};
}
