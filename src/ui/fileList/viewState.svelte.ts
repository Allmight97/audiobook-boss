import type { AudioFile } from '../../types/audio';

type FileListViewState = {
	files: AudioFile[];
	selectedIndices: number[];
	sortLabel: string;
	showSortButton: boolean;
	showClearButton: boolean;
	sortDisabled: boolean;
	clearDisabled: boolean;
	orderLockVisible: boolean;
	combinedSizeText: string;
};

const EMPTY_STATE: FileListViewState = {
	files: [],
	selectedIndices: [],
	sortLabel: 'Sort: A-Z',
	showSortButton: false,
	showClearButton: false,
	sortDisabled: false,
	clearDisabled: false,
	orderLockVisible: false,
	combinedSizeText: '--- MB',
};

export const fileListViewState = $state<FileListViewState>({ ...EMPTY_STATE });

export function setFileListViewFiles(files: AudioFile[]): void {
	fileListViewState.files = files;
}

export function setFileListViewSelection(indices: Set<number>): void {
	fileListViewState.selectedIndices = Array.from(indices);
}

export function setFileListSortLabel(ascending: boolean): void {
	fileListViewState.sortLabel = ascending ? 'Sort: A-Z' : 'Sort: Z-A';
}

export function setFileListControlsState(options: {
	showSortButton: boolean;
	showClearButton: boolean;
	sortDisabled: boolean;
	clearDisabled: boolean;
}): void {
	fileListViewState.showSortButton = options.showSortButton;
	fileListViewState.showClearButton = options.showClearButton;
	fileListViewState.sortDisabled = options.sortDisabled;
	fileListViewState.clearDisabled = options.clearDisabled;
}

export function setFileListOrderLockVisible(visible: boolean): void {
	fileListViewState.orderLockVisible = visible;
}

export function resetFileListViewState(): void {
	fileListViewState.files = EMPTY_STATE.files;
	fileListViewState.selectedIndices = EMPTY_STATE.selectedIndices;
	fileListViewState.sortLabel = EMPTY_STATE.sortLabel;
	fileListViewState.showSortButton = EMPTY_STATE.showSortButton;
	fileListViewState.showClearButton = EMPTY_STATE.showClearButton;
	fileListViewState.sortDisabled = EMPTY_STATE.sortDisabled;
	fileListViewState.clearDisabled = EMPTY_STATE.clearDisabled;
	fileListViewState.orderLockVisible = EMPTY_STATE.orderLockVisible;
	fileListViewState.combinedSizeText = EMPTY_STATE.combinedSizeText;
}

export function setFileListCombinedSizeText(value: string): void {
	fileListViewState.combinedSizeText = value;
}
