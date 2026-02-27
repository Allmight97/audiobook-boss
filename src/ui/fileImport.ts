import { clearFileImportError, setFileImportHasFiles } from './fileImport/state.svelte';

export function initFileImport(): void {
	clearFileImportError();
	updateDropZoneState(false);
}

export function updateDropZoneState(hasFiles: boolean): void {
	setFileImportHasFiles(hasFiles);
}
