import { formatFileSize, type AudioFile } from '../../types/audio';
import {
	getCurrentFileList,
	getSelectedFileIndices,
	getSortAscending,
	isOrderLocked,
} from './state.svelte';

export function readFileListViewFiles(): AudioFile[] {
	const fileList = getCurrentFileList();
	if (!fileList) {
		return [];
	}
	return [...fileList.files];
}

export function readFileListSelectedIndices(): number[] {
	return Array.from(getSelectedFileIndices());
}

export function readFileListSortLabel(): string {
	return getSortAscending() ? 'Sort: A-Z' : 'Sort: Z-A';
}

export function readFileListControlsSnapshot(): {
	showSortButton: boolean;
	showClearButton: boolean;
	sortDisabled: boolean;
	clearDisabled: boolean;
} {
	const fileList = getCurrentFileList();
	const locked = isOrderLocked();
	if (!fileList) {
		return {
			showSortButton: false,
			showClearButton: false,
			sortDisabled: locked,
			clearDisabled: locked,
		};
	}

	return {
		showSortButton: fileList.files.length > 1,
		showClearButton: fileList.files.length > 0,
		sortDisabled: locked,
		clearDisabled: locked,
	};
}

export function readFileListOrderLockVisible(): boolean {
	return isOrderLocked();
}

export function readCombinedSizeText(): string {
	const fileList = getCurrentFileList();
	if (!fileList) {
		return '--- MB';
	}
	return formatFileSize(fileList.totalSize);
}
