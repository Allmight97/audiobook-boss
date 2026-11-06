import type { UnlistenFn } from '@tauri-apps/api/event';

/**
 * A lifecycle-owned collection of Tauri event unlisteners. If the group is
 * disposed before a pending `listen` promise resolves, the resolved unlisten
 * runs at once; otherwise `dispose()` runs it once.
 */
export interface SubscriptionGroup {
	/**
	 * Register an unlisten — or a pending `listen` promise that resolves to one —
	 * with the group. If the group is already disposed when the unlisten becomes
	 * available, it is invoked immediately instead of stored. Resolves once the
	 * registration settles, so a caller needing subscribe-before-X ordering can await.
	 */
	add: (registration: Promise<UnlistenFn> | UnlistenFn) => Promise<void>;
	/** Invoke and drop every registered unlisten exactly once; idempotent. */
	dispose: () => void;
	/** True once `dispose()` has run; further `add()`s unlisten immediately. */
	readonly disposed: boolean;
}

export function createSubscriptionGroup(): SubscriptionGroup {
	let disposed = false;
	const unlisteners: UnlistenFn[] = [];

	const register = (unlisten: UnlistenFn): void => {
		if (disposed) {
			unlisten();
			return;
		}
		unlisteners.push(unlisten);
	};

	return {
		get disposed() {
			return disposed;
		},
		async add(registration) {
			const unlisten = typeof registration === 'function' ? registration : await registration;
			register(unlisten);
		},
		dispose() {
			if (disposed) {
				return;
			}
			disposed = true;
			for (const unlisten of unlisteners.splice(0)) {
				unlisten();
			}
		},
	};
}
