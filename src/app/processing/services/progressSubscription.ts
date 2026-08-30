import { tauriClient } from '../../../lib/tauri/client';
import { EVENTS, type ProcessingProgressEvent, type ProcessingQueueEvent } from '../../../types/events';
import {
	createSubscriptionGroup,
	type SubscriptionGroup,
} from '../../../lib/tauri/subscriptionGroup';

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

async function listenForProgressEvents(
	onProgress: (event: ProcessingProgressEvent) => void,
): Promise<Unlisten> {
	return tauriClient.listen(EVENTS.PROGRESS, (event) => {
		onProgress(event.payload);
	});
}

async function listenForQueueEvents(
	onQueue: (event: ProcessingQueueEvent) => void,
): Promise<Unlisten> {
	return tauriClient.listen(EVENTS.QUEUE, (event) => {
		onQueue(event.payload);
	});
}

export function createProgressSubscription({
	onProgress,
	onQueue,
	listenForProgressEvents: subscribeToProgress = listenForProgressEvents,
	listenForQueueEvents: subscribeToQueue = listenForQueueEvents,
	eventTarget = getDefaultEventTarget(),
}: CreateProgressSubscriptionOptions): ProgressSubscriptionOwner {
	// One subscription group per start cycle. Its `disposed` flag is the
	// stale-start guard (a stop/dispose between a `listen` call and its resolution
	// unlistens the late arrival), replacing the previous hand-rolled token.
	let currentGroup: SubscriptionGroup | null = null;
	let subscribed = false;
	let beforeUnloadAttached = false;
	let disposed = false;
	let startPromise: Promise<void> | null = null;

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
		currentGroup?.dispose();
		currentGroup = null;
		subscribed = false;
		startPromise = null;
		detachBeforeUnload();
	};

	const handleBeforeUnload = () => {
		teardown();
	};

	const ensureSubscribed = async () => {
		if (disposed) {
			throw new Error('Progress subscription has been disposed.');
		}

		if (subscribed) {
			return;
		}

		if (startPromise) {
			return startPromise;
		}

		const group = createSubscriptionGroup();
		currentGroup = group;
		attachBeforeUnload();

		let pendingStart: Promise<void> | null = null;
		pendingStart = (async () => {
			try {
				await group.add(subscribeToProgress(onProgress));
				await group.add(subscribeToQueue(onQueue));
				// A stop/dispose during startup disposed this group and already
				// unlistened any late arrivals; do not mark the cycle subscribed.
				if (group.disposed) {
					return;
				}
				subscribed = true;
			} catch (error) {
				group.dispose();
				if (currentGroup === group) {
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
