export type TypingEcho<Key> = {
	show(key: Key, value: string, send: () => Promise<unknown>): void;
	valueOf(key: Key): string | undefined;
	drop(key: Key): void;
	clear(): void;
};

type Entry = { readonly value: string; readonly typedIn: readonly unknown[] };

function sameScope(left: readonly unknown[], right: readonly unknown[]): boolean {
	return left.length === right.length && left.every((part, index) => Object.is(part, right[index]));
}

export function createTypingEcho<Key>(deps: {
	readonly scopeOf: () => readonly unknown[];
	readonly changed: () => void;
}): TypingEcho<Key> {
	const unconfirmed = new Map<Key, Entry>();

	return {
		show(key, value, send) {
			const entry: Entry = { value, typedIn: deps.scopeOf() };
			unconfirmed.set(key, entry);
			deps.changed();
			void send().finally(() => {
				if (unconfirmed.get(key) !== entry) return;
				unconfirmed.delete(key);
				deps.changed();
			});
		},
		valueOf(key) {
			const entry = unconfirmed.get(key);
			return entry && sameScope(entry.typedIn, deps.scopeOf()) ? entry.value : undefined;
		},
		drop(key) {
			unconfirmed.delete(key);
			deps.changed();
		},
		clear() {
			unconfirmed.clear();
			deps.changed();
		},
	};
}
