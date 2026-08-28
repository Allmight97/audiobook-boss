import { pathBasename } from '../../../lib/path/basename';
import { formatFileSize, type AudioFile } from '../../../types/audio';
import { companionSummaryForInputIds } from '../../remoteSource';
import { displayedArtistForFile, displayedTitleForFile } from '../viewState.svelte';
import { fileListSession, getImportOrdinal, type FileListSession } from './session';

export type FileListInspectorView = {
	contextText: string;
	contextVariant: 'empty' | 'single' | 'multi';
	contextDetail: string;
	bitrateText: string;
	sampleRateText: string;
	channelsText: string;
	codecText: string;
	decoderText: string;
	fileSizeText: string;
	companionsText: string;
	companionsTitle: string;
};

export type FileListView = {
	files: AudioFile[];
	selectedIndices: number[];
	selectedFileIndex: number;
	sortLabel: string;
	sortState: FileListSession['sortDirection'];
	showSortButton: boolean;
	showClearButton: boolean;
	sortDisabled: boolean;
	clearDisabled: boolean;
	orderLockVisible: boolean;
	orderDiffersFromImport: boolean;
	combinedSizeText: string;
	inspector: FileListInspectorView;
};

const EMPTY_INSPECTOR: FileListInspectorView = {
	contextText: 'No file selected',
	contextVariant: 'empty',
	contextDetail: '',
	bitrateText: '---',
	sampleRateText: '---',
	channelsText: '---',
	codecText: '---',
	decoderText: '---',
	fileSizeText: '---',
	companionsText: '---',
	companionsTitle: '',
};

function orderDiffersFromImport(files: readonly AudioFile[]): boolean {
	if (files.length <= 1) return false;
	let previous = -1;
	for (const file of files) {
		const ordinal = getImportOrdinal(file.path);
		if (ordinal === undefined) return false;
		if (ordinal < previous) return true;
		previous = ordinal;
	}
	return false;
}

function optionalText(value: string | undefined): string {
	const trimmed = value?.trim();
	return trimmed && trimmed.length > 0 ? trimmed : 'N/A';
}

function sharedText(
	files: readonly AudioFile[],
	pick: (file: AudioFile) => string | undefined,
): string {
	if (files.length === 0) return '---';
	const values = files.map((file) => {
		const value = pick(file);
		return typeof value === 'string' && value.trim().length > 0 ? value.trim() : null;
	});
	if (values.some((value) => value === null)) return 'Mixed';
	return new Set(values).size === 1 ? (values[0] ?? 'Mixed') : 'Mixed';
}

function inspectorFor(session: FileListSession): FileListInspectorView {
	const files = session.currentFileList?.files ?? [];
	const selected = [...session.selectedFileIndices]
		.sort((a, b) => a - b)
		.map((index) => files[index])
		.filter((file): file is AudioFile => Boolean(file));
	if (selected.length === 0) return EMPTY_INSPECTOR;
	if (selected.length > 1) {
		const companions = companionSummaryForInputIds(selected.map((file) => file.inputId));
		return {
			...EMPTY_INSPECTOR,
			contextText: `${selected.length} files selected`,
			contextVariant: 'multi',
			codecText: sharedText(selected, (file) => file.codecLabel),
			decoderText: sharedText(selected, (file) => file.selectedDecoder),
			companionsText: companions.text,
			companionsTitle: companions.title,
		};
	}
	const file = selected[0];
	const companions = companionSummaryForInputIds([file.inputId]);
	return {
		contextText: pathBasename(file.path, { fallback: 'path' }),
		contextVariant: 'single',
		contextDetail:
			session.selectedFileIndex >= 0 ? `${session.selectedFileIndex + 1} of ${files.length}` : '',
		bitrateText:
			file.isValid && file.bitrate ? `${file.bitrate} kb/s` : file.isValid ? 'N/A' : '---',
		sampleRateText:
			file.isValid && file.sampleRate ? `${file.sampleRate} Hz` : file.isValid ? 'N/A' : '---',
		channelsText:
			file.isValid && file.channels ? `${file.channels} ch` : file.isValid ? 'N/A' : '---',
		codecText: file.isValid ? optionalText(file.codecLabel) : '---',
		decoderText: file.isValid ? optionalText(file.selectedDecoder) : '---',
		fileSizeText:
			file.isValid && file.size ? formatFileSize(file.size) : file.isValid ? 'N/A' : '---',
		companionsText: companions.text,
		companionsTitle: companions.title,
	};
}

export function readFileListView(): FileListView {
	const session = fileListSession();
	const files = session.currentFileList ? [...session.currentFileList.files] : [];
	const locked = session.orderLocked;
	return {
		files,
		selectedIndices: [...session.selectedFileIndices],
		selectedFileIndex: session.selectedFileIndex,
		sortLabel: session.sortDirection === 'descending' ? 'Sort: Z-A' : 'Sort: A-Z',
		sortState: session.sortDirection,
		showSortButton: files.length > 1,
		showClearButton: files.length > 0,
		sortDisabled: locked,
		clearDisabled: locked,
		orderLockVisible: locked,
		orderDiffersFromImport: orderDiffersFromImport(files),
		combinedSizeText: session.currentFileList
			? formatFileSize(session.currentFileList.totalSize)
			: '--- MB',
		inspector: inspectorFor(session),
	};
}

export { displayedArtistForFile, displayedTitleForFile };
