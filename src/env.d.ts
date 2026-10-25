// Vitest runs in Node. The Progress test reads a stylesheet, and the app
// typecheck does not include @types/node.
declare module 'node:fs' {
	export function readFileSync(path: string, encoding: 'utf8'): string;
}

interface TauriInternals {
	[key: string]: unknown;
}

interface Window {
	__TAURI_INTERNALS__?: TauriInternals;
	currentCoverArt?: number[];
}
