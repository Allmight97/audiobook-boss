interface TauriInternals {
	[key: string]: unknown;
}

interface Window {
	__TAURI_INTERNALS__?: TauriInternals;
	currentCoverArt?: number[];
}
