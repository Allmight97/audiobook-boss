import { writable, type Readable } from 'svelte/store';

/** Owner-internal writable; the save workflow is its only writer. */
export const metadataSaveInProgressStore = writable(false);

/** Public readonly view for UI surfaces that render save-in-progress state. */
export const metadataSaveInProgress: Readable<boolean> = {
	subscribe: metadataSaveInProgressStore.subscribe,
};
