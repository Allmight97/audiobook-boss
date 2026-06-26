import type { UnlistenFn } from '@tauri-apps/api/event';

/**
 * A lifecycle-owned collection of Tauri event unlisteners.
 *
 * Centralizes the dispose / late-arrival race that UI islands otherwise
 * re-implement per subscription: if the group is disposed before a pending
 * `listen` promise resolves, the resolved unlisten is invoked immediately so the
 * listener cannot leak; otherwise it is collected and invoked once on `dispose()`.
 *
 * This is a deliberate frontend utility surface (see `AGENTS.md`); it is not a
 * `tauriClient` IPC method.
 */
export interface SubscriptionGroup {
	/**
	 * Register an unlisten — or a pending `listen` promise that resolves to one —
	 * with the group. If the group is already disposed when the unlisten becomes
	 * available, it is invoked immediately instead of stored. Resolves once the
	 * registration settles, so a caller needing subscribe-before-X ordering can await.
	 */
	add(registration: Promise<UnlistenFn> | UnlistenFn): Promise<void>;
	/** Invoke and drop every registered unlisten exactly once; idempotent. */
	dispose(): void;
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
