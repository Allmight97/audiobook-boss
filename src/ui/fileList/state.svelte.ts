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

function setSelectedFileIndexInRange(index: number, fileCount: number): void {
	fileListSessionState.selectedFileIndex = index >= 0 && index < fileCount ? index : -1;
}

export function setCurrentFileList(fileList: FileListInfo | null): void {
	fileListSessionState.currentFileList = fileList;

	if (!fileList) {
		fileListSessionState.selectedFileIndex = -1;
		fileListSessionState.selectedFileIndices = new Set<number>();
		return;
	}

	const fileCount = fileList.files.length;
	const nextSelectedIndices = Array.from(fileListSessionState.selectedFileIndices).filter(
		(index) => index >= 0 && index < fileCount,
	);
	fileListSessionState.selectedFileIndices = createSelectedIndexSet(nextSelectedIndices);
	setSelectedFileIndexInRange(fileListSessionState.selectedFileIndex, fileCount);
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

export function hasFiles(): boolean {
	return (
		fileListSessionState.currentFileList !== null &&
		fileListSessionState.currentFileList.files.length > 0
	);
}

export function isValidIndex(index: number): boolean {
	if (!fileListSessionState.currentFileList) return false;
	return index >= 0 && index < fileListSessionState.currentFileList.files.length;
}

export function getFileCount(): number {
	return fileListSessionState.currentFileList?.files.length || 0;
}
