import type { UnlistenFn } from '@tauri-apps/api/event';

export interface SubscriptionGroup {
	add: (registration: Promise<UnlistenFn> | UnlistenFn) => Promise<void>;
	dispose: () => void;
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
