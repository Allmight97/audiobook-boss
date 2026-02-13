declare module '*.svelte' {
	import type { Component } from 'svelte';
	const component: Component;
	export default component;
}

interface TauriInternals {
	[key: string]: unknown;
}

interface Window {
	__TAURI_INTERNALS__?: TauriInternals;
	currentCoverArt?: number[];
}
