import { get, writable } from 'svelte/store';

export const metadataSaveInProgressStore = writable(false);

export function setMetadataSaveInProgress(inProgress: boolean): void {
	metadataSaveInProgressStore.set(inProgress);
}

export function isMetadataSaveInProgress(): boolean {
	return get(metadataSaveInProgressStore);
}
