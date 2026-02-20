import { mount, unmount } from 'svelte';
import FileImportIsland from './fileImport/FileImportIsland.svelte';
import { clearFileImportError, setFileImportHasFiles } from './fileImport/state.svelte';

const FILE_IMPORT_ROOT_ID = 'file-import-root';

let mountedFileImportRoot: HTMLElement | null = null;
let mountedFileImportIsland: Parameters<typeof unmount>[0] | null = null;

export function initFileImport(): void {
	mountFileImportIsland();
	clearFileImportError();
	updateDropZoneState(false);
}

function mountFileImportIsland(): void {
	const importRoot = document.getElementById(FILE_IMPORT_ROOT_ID);
	if (!importRoot) return;

	if (
		mountedFileImportIsland &&
		mountedFileImportRoot === importRoot &&
		importRoot.childElementCount > 0
	) {
		return;
	}

	if (mountedFileImportIsland) {
		void unmount(mountedFileImportIsland);
		mountedFileImportIsland = null;
	}

	mountedFileImportIsland = mount(FileImportIsland, { target: importRoot });
	mountedFileImportRoot = importRoot;
}

export function updateDropZoneState(hasFiles: boolean): void {
	setFileImportHasFiles(hasFiles);
}
