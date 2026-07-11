import {
	formatDuration,
	formatFileSize,
	type AudioChapter,
	type AudioFile,
} from '../../types/audio';
import {
	getCurrentFileList,
	getSelectedFileIndex,
	getSelectedFileIndices,
	getSortAscending,
	isOrderLocked,
} from './state.svelte';

type FileWorkActivity = {
	status: 'queued' | 'running' | 'done' | 'skipped' | 'cancelled' | 'failed';
};

export type ReadWorkActivityByInputId = () => ReadonlyMap<string, FileWorkActivity>;

export type FileListStatusBadge = {
	label: 'Ready' | 'Queued' | 'Running' | 'Done' | 'Skipped' | 'Cancelled' | 'Failed' | 'Error';
	variant: 'ok' | 'info' | 'muted' | 'warn';
	isError: boolean;
};

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

export function readFileListStatusBadge(
	file: AudioFile,
	readWorkActivityByInputId?: ReadWorkActivityByInputId,
): FileListStatusBadge {
	if (!file.isValid) {
		return { label: 'Error', variant: 'warn', isError: true };
	}

	const activity = file.inputId ? readWorkActivityByInputId?.().get(file.inputId) : undefined;
	switch (activity?.status) {
		case 'queued':
			return { label: 'Queued', variant: 'muted', isError: false };
		case 'running':
			return { label: 'Running', variant: 'info', isError: false };
		case 'done':
			return { label: 'Done', variant: 'ok', isError: false };
		case 'skipped':
			return { label: 'Skipped', variant: 'muted', isError: false };
		case 'cancelled':
			return { label: 'Cancelled', variant: 'muted', isError: false };
		case 'failed':
			return { label: 'Failed', variant: 'warn', isError: true };
		default:
			return { label: 'Ready', variant: 'muted', isError: false };
	}
}

export function readCombinedSizeText(): string {
	const fileList = getCurrentFileList();
	if (!fileList) {
		return '--- MB';
	}
	return formatFileSize(fileList.totalSize);
}

export function readFileListCount(): number {
	return getCurrentFileList()?.files.length ?? 0;
}

/** Read-only active-file chapter facts for the metadata-surface Chapters tab. */
export function readActiveFileChapters(): readonly AudioChapter[] {
	const fileList = getCurrentFileList();
	const selectedIndex = getSelectedFileIndex();
	return fileList?.files[selectedIndex]?.chapters ?? [];
}

export function readCombinedDurationText(): string {
	return formatDuration(getCurrentFileList()?.totalDuration);
}
