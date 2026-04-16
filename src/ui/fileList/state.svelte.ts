import type { FileListInfo } from '../../types/audio';

type FileListSessionState = {
	currentFileList: FileListInfo | null;
	selectedFileIndex: number;
	selectedFileIndices: Set<number>;
	sortAscending: boolean;
	orderLocked: boolean;
};

export const fileListSessionState = $state<FileListSessionState>({
	currentFileList: null,
	selectedFileIndex: -1,
	selectedFileIndices: new Set<number>(),
	sortAscending: true,
	orderLocked: false,
});

function createSelectedIndexSet(indices: Set<number> | number[]): Set<number> {
	const next = new Set<number>();
	for (const index of Array.isArray(indices) ? indices : Array.from(indices)) {
		next.add(index);
	}
	return next;
}

export function setCurrentFileList(fileList: FileListInfo | null): void {
	fileListSessionState.currentFileList = fileList;

	fileListSessionState.selectedFileIndex = -1;
	fileListSessionState.selectedFileIndices = new Set<number>();

	if (!fileList) {
		return;
	}
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

export function setSelectedFileIndices(indices: Set<number> | number[]): void {
	fileListSessionState.selectedFileIndices = createSelectedIndexSet(indices);
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

export function getSortAscending(): boolean {
	return fileListSessionState.sortAscending;
}

export function setSortAscending(ascending: boolean): void {
	fileListSessionState.sortAscending = ascending;
}

export function setOrderLocked(locked: boolean): void {
	fileListSessionState.orderLocked = locked;
}

export function isOrderLocked(): boolean {
	return fileListSessionState.orderLocked;
}
