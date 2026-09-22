import {
	formatDuration,
	formatFileSize,
	formatAudioBitrate,
	type AudioFile,
} from '../../types/audio';
import {
	fileIdentityKey,
	orderDiffersFromImport,
	type InputSessionState,
	type InputView,
} from './types';

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
	const sourceFiles = files.flatMap(
		(file) => session.titleSourcesByIdentity[fileIdentityKey(file)] ?? [file],
	);
	const locked = session.orderLocked;
	const differs = orderDiffersFromImport(files, session.importOrdinalByPath);
	return {
		files,
		sourceFiles,
		selectedSourceFiles: session.selectedIndices.flatMap((index) => {
			const file = files[index];
			return file ? (session.titleSourcesByIdentity[fileIdentityKey(file)] ?? [file]) : [];
		}),
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
