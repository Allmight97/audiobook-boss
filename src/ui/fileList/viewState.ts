import { formatFileSize, type AudioFile } from '../../types/audio';
import {
	getMetadataForFile,
	getMetadataIntentPatchForFile,
	isUsableMetadataCache,
} from '../metadataSession';
import { pathBasename } from '../../lib/path/basename';
import {
	getCurrentFileList,
	getImportOrdinal,
	getSelectedFileIndices,
	getSortDirection,
	isOrderLocked,
} from './state';

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
	return getSortDirection() === 'descending' ? 'Sort: Z-A' : 'Sort: A-Z';
}

export function readFileListSortState(): 'none' | 'ascending' | 'descending' {
	return getSortDirection();
}

export function readFileListOrderDiffersFromImport(): boolean {
	const fileList = getCurrentFileList();
	if (!fileList || fileList.files.length <= 1) return false;
	let previous = -1;
	for (const file of fileList.files) {
		const ordinal = getImportOrdinal(file.path);
		if (ordinal === undefined) return false;
		if (ordinal < previous) return true;
		previous = ordinal;
	}
	return false;
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

export function displayedTitleForFile(file: AudioFile): string {
	const metadata = getMetadataForFile(file.path);
	const titleIntent = getMetadataIntentPatchForFile(file.path)?.title;
	if (titleIntent?.op === 'clear') return pathBasename(file.path, { fallback: 'path' });
	if (typeof metadata?.title === 'string' && metadata.title.trim().length > 0) {
		return metadata.title;
	}
	if (!isUsableMetadataCache(metadata) || metadata?.title === undefined) {
		return file.tagTitle || pathBasename(file.path, { fallback: 'path' });
	}
	return pathBasename(file.path, { fallback: 'path' });
}

export function displayedArtistForFile(file: AudioFile): string | null {
	const metadata = getMetadataForFile(file.path);
	const artistIntent = getMetadataIntentPatchForFile(file.path)?.artist;
	if (artistIntent?.op === 'clear') return null;
	if (typeof metadata?.artist === 'string' && metadata.artist.trim().length > 0) {
		return metadata.artist;
	}
	if (!isUsableMetadataCache(metadata) || metadata?.artist === undefined) {
		return file.tagArtist || null;
	}
	return null;
}

export function readCombinedSizeText(): string {
	const fileList = getCurrentFileList();
	if (!fileList) {
		return '--- MB';
	}
	return formatFileSize(fileList.totalSize);
}
