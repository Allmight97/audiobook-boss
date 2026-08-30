import { formatDuration, formatFileSize, type AudioFile } from '../../types/audio';
import { orderDiffersFromImport, type InputSessionState, type InputView } from './types';

export function displayedTitleForFile(file: AudioFile): string {
	if (file.tagTitle?.trim()) {
		return file.tagTitle;
	}
	const segments = file.path.split(/[\\/]/).filter((segment) => segment !== '');
	return segments[segments.length - 1] ?? file.path;
}

export function displayedArtistForFile(file: AudioFile): string {
	return file.tagArtist?.trim() ?? '';
}

export function formatFileDetails(file: AudioFile): string {
	const artist = displayedArtistForFile(file);
	const artistPrefix = artist ? `${artist} • ` : '';
	const chapterSuffix = file.chapters?.length
		? ` • ${file.chapters.length} chapter${file.chapters.length === 1 ? '' : 's'}`
		: '';
	if (file.isValid && file.duration && file.size) {
		return `${artistPrefix}${formatDuration(file.duration)} • ${formatFileSize(file.size)} • ${file.format}${chapterSuffix}`;
	}
	return `Error: ${file.error || 'Invalid file'}`;
}

export function toInputView(session: InputSessionState): InputView {
	const files = session.fileList?.files ?? [];
	const locked = session.orderLocked;
	const differs = orderDiffersFromImport(files, session.importOrdinalByPath);
	return {
		files,
		selectedIndices: session.selectedIndices,
		selectedAnchor: session.selectedAnchor,
		fileCount: files.length,
		hasFiles: files.length > 0,
		orderLocked: locked,
		errorMessage: session.errorMessage,
		isDragOver: session.isDragOver,
		supportText: session.supportText,
		sortDirection: session.sortDirection,
		sortLabel: session.sortDirection === 'descending' ? 'Sort: Z-A' : 'Sort: A-Z',
		orderDiffersFromImport: differs,
		showSortButton: files.length > 1,
		showClearButton: files.length > 0,
		showRestoreImportOrder: differs && !locked,
		totalDurationSeconds: session.fileList?.totalDuration ?? 0,
	};
}
