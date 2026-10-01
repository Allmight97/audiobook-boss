import {
	formatDuration,
	formatFileSize,
	formatAudioBitrate,
	type AudioFile,
} from '../../types/audio';
import { toUserMessage } from '../../lib/tauri/appError';
import type { InputNotice, SessionSelection, SessionTitles } from '../../types/session';
import { fileIdentityKey, type InputView } from './types';

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

/** Words the engine's reason an import added nothing. */
export function inputNoticeText(notice: InputNotice | null): string {
	switch (notice?.kind) {
		case undefined:
			return '';
		case 'orderLocked':
			return 'Order locked while processing. Wait for completion to add files.';
		case 'noSupportedFiles':
			return `No supported audio files found. Please use ${notice.formatsText} files.`;
		case 'duplicatesOnly':
			return 'No new files added. All analyzed files were already in the list.';
		case 'discoveryFailed':
			return toUserMessage(notice.error, {
				fallback: 'Failed to discover audio files. Please try again.',
			});
		case 'analysisFailed':
			return toUserMessage(notice.error, {
				fallback: 'Failed to analyze files. Please try again.',
			});
	}
}

/** What the views show: the engine's titles and selection plus view-local state. */
export function toInputView(
	titles: SessionTitles,
	selection: SessionSelection,
	local: {
		readonly errorMessage: string;
		readonly isDragOver: boolean;
		readonly supportText: string;
	},
): InputView {
	const files = titles.files;
	const sourcesOf = (file: AudioFile) =>
		titles.titleSourcesByIdentity[fileIdentityKey(file)] ?? [file];
	const sourceFiles = files.flatMap(sourcesOf);
	const locked = titles.orderLocked;
	return {
		files,
		sourceFiles,
		selectedSourceFiles: selection.selectedIndices.flatMap((index) => {
			const file = files[index];
			return file ? sourcesOf(file) : [];
		}),
		selectedIndices: selection.selectedIndices,
		selectedAnchor: selection.selectedAnchor ?? -1,
		fileCount: files.length,
		hasFiles: files.length > 0,
		orderLocked: locked,
		errorMessage: local.errorMessage || inputNoticeText(titles.notice),
		isDragOver: local.isDragOver,
		supportText: local.supportText,
		sortDirection: titles.sortDirection,
		sortLabel: titles.sortDirection === 'descending' ? 'Sort: Z-A' : 'Sort: A-Z',
		orderDiffersFromImport: titles.orderDiffersFromImport,
		showSortButton: files.length > 1,
		showClearButton: files.length > 0,
		showRestoreImportOrder: titles.orderDiffersFromImport && !locked,
		totalDurationSeconds: sourceFiles.reduce((sum, file) => sum + (file.duration ?? 0), 0),
	};
}

export function formatAudioProperties(file: AudioFile): string {
	const bitrate = file.bitrate ? formatAudioBitrate(file.bitrate) : 'Bitrate unknown';
	const rate = file.sampleRate ? `${file.sampleRate / 1000} kHz` : 'Sample rate unknown';
	const channels =
		file.channels === 1
			? 'Mono'
			: file.channels === 2
				? 'Stereo'
				: file.channels
					? `${file.channels} channels`
					: 'Channels unknown';
	return [bitrate, rate, channels, file.codecLabel?.trim() || 'Codec unknown'].join(' · ');
}
