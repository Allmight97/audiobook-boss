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
	getSortDirection,
	isOrderLocked,
} from './state.svelte';
import { pathBasename } from '../../lib/path/basename';
import { getMetadataForFile } from '../metadataSession';

type FileWorkActivity = {
	status: 'queued' | 'running' | 'done' | 'skipped' | 'cancelled' | 'failed';
};

export type ReadWorkActivityByInputId = () => ReadonlyMap<string, FileWorkActivity>;

export type FileListStatusBadge = {
	label: 'ready' | 'queued' | 'running' | 'done' | 'skipped' | 'cancelled' | 'failed' | 'error';
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

export function readFileListSortState(): 'none' | 'ascending' | 'descending' {
	return getSortDirection();
}

export function readFileListOrderLockVisible(): boolean {
	return isOrderLocked();
}

export function readFileListStatusBadge(
	file: AudioFile,
	readWorkActivityByInputId?: ReadWorkActivityByInputId,
): FileListStatusBadge {
	if (!file.isValid) {
		return { label: 'error', variant: 'warn', isError: true };
	}

	const activity = file.inputId ? readWorkActivityByInputId?.().get(file.inputId) : undefined;
	switch (activity?.status) {
		case 'queued':
			return { label: 'queued', variant: 'muted', isError: false };
		case 'running':
			return { label: 'running', variant: 'info', isError: false };
		case 'done':
			return { label: 'done', variant: 'ok', isError: false };
		case 'skipped':
			return { label: 'skipped', variant: 'muted', isError: false };
		case 'cancelled':
			return { label: 'cancelled', variant: 'muted', isError: false };
		case 'failed':
			return { label: 'failed', variant: 'warn', isError: true };
		default:
			return { label: 'ready', variant: 'muted', isError: false };
	}
}

export function displayedFileNameForFile(file: AudioFile): string {
	return pathBasename(file.path, { fallback: 'path' });
}

/** Display-only title beside the filename; session truth wins once an entry exists. */
export function displayedSecondaryTitleForFile(file: AudioFile): string | null {
	const metadata = getMetadataForFile(file.path);
	return (metadata === undefined ? file.tagTitle : metadata.title) || null;
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
