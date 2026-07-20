import type { AudioFile, FileListInfo } from '../../types/audio';

type FileListSessionState = {
	currentFileList: FileListInfo | null;
	selectedFileIndex: number;
	selectedFileIndices: Set<number>;
	sortDirection: FileListSortDirection;
	orderLocked: boolean;
};

export type FileListSortDirection = 'none' | 'ascending' | 'descending';

type OrderLockListener = (locked: boolean) => void;

export const fileListSessionState = $state<FileListSessionState>({
	currentFileList: null,
	selectedFileIndex: -1,
	selectedFileIndices: new Set<number>(),
	sortDirection: 'none',
	orderLocked: false,
});

const orderLockListeners = new Set<OrderLockListener>();

// Import (arrival) order, path-keyed to match append/dedupe identity. A plain
// Map is reactivity-safe: every order mutation also replaces currentFileList,
// so derived reads re-run at the right times. Assigned only by display/append
// actions — never by setCurrentFileList, which cannot tell replace from append.
const importOrdinalByPath = new Map<string, number>();
let nextImportOrdinal = 0;

export function resetImportOrder(files: AudioFile[]): void {
	importOrdinalByPath.clear();
	nextImportOrdinal = 0;
	for (const file of files) {
		importOrdinalByPath.set(file.path, nextImportOrdinal++);
	}
}

/** Assigns fresh ordinals to paths not seen yet; existing paths keep theirs. */
export function recordImportOrder(files: AudioFile[]): void {
	for (const file of files) {
		if (!importOrdinalByPath.has(file.path)) {
			importOrdinalByPath.set(file.path, nextImportOrdinal++);
		}
	}
}

export function removeImportOrdinal(path: string): void {
	importOrdinalByPath.delete(path);
}

export function getImportOrdinal(path: string): number | undefined {
	return importOrdinalByPath.get(path);
}

export function setCurrentFileList(fileList: FileListInfo | null): void {
	fileListSessionState.currentFileList = fileList;

	fileListSessionState.selectedFileIndex = -1;
	fileListSessionState.selectedFileIndices = new Set<number>();
	fileListSessionState.sortDirection = 'none';
}

export function setSelectedIndex(index: number): void {
	fileListSessionState.selectedFileIndex = index;
}

export function getCurrentFileList(): FileListInfo | null {
	return fileListSessionState.currentFileList;
}

export function getSelectedFileIndex(): number {
	return fileListSessionState.selectedFileIndex;
}

export function getSelectedFileIndices(): Set<number> {
	return new Set(fileListSessionState.selectedFileIndices);
}

export function getSelectedFiles(): AudioFile[] {
	const fileList = getCurrentFileList();
	if (!fileList) {
		return [];
	}
	return Array.from(getSelectedFileIndices())
		.map((index) => fileList.files[index])
		.filter((file): file is AudioFile => Boolean(file));
}

export function setSelectedFileIndices(indices: Set<number> | number[]): void {
	fileListSessionState.selectedFileIndices = new Set(indices);
}

export function addToSelectedIndices(index: number): void {
	fileListSessionState.selectedFileIndices = new Set(fileListSessionState.selectedFileIndices);
	fileListSessionState.selectedFileIndices.add(index);
}

export function removeFromSelectedIndices(index: number): void {
	fileListSessionState.selectedFileIndices = new Set(fileListSessionState.selectedFileIndices);
	fileListSessionState.selectedFileIndices.delete(index);
}

export function clearSelectedIndices(): void {
	fileListSessionState.selectedFileIndices = new Set<number>();
}

export function getSortDirection(): FileListSortDirection {
	return fileListSessionState.sortDirection;
}

export function setSortDirection(direction: FileListSortDirection): void {
	fileListSessionState.sortDirection = direction;
}

export function setOrderLocked(locked: boolean): void {
	if (fileListSessionState.orderLocked === locked) {
		return;
	}

	fileListSessionState.orderLocked = locked;
	for (const listener of [...orderLockListeners]) {
		listener(locked);
	}
}

export function isOrderLocked(): boolean {
	return fileListSessionState.orderLocked;
}

export function onOrderLockChange(listener: OrderLockListener): () => void {
	orderLockListeners.add(listener);
	return () => {
		orderLockListeners.delete(listener);
	};
}
