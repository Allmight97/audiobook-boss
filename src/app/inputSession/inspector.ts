import { formatFileSize, type AudioFile } from '../../types/audio';
import { pathBasename } from '../../lib/path/basename';
import type { InputSessionState, InputView } from './types';

export type InspectorContextVariant = 'empty' | 'single' | 'multi';

export type InspectorView = {
	readonly contextText: string;
	readonly contextVariant: InspectorContextVariant;
	readonly contextDetail: string;
	readonly bitrateText: string;
	readonly sampleRateText: string;
	readonly channelsText: string;
	readonly codecText: string;
	readonly decoderText: string;
	readonly fileSizeText: string;
	readonly companionsText: string;
	readonly companionsTitle: string;
	readonly combinedSizeText: string;
};

export const EMPTY_INSPECTOR_VIEW: InspectorView = {
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
	combinedSizeText: '--- MB',
};

export function toInspectorView(
	session: InputSessionState,
	companionSummaryForInputIds: (inputIds: ReadonlyArray<string | undefined>) => {
		readonly text: string;
		readonly title: string;
	},
): InspectorView {
	return inspectorFromFiles(
		session.fileList?.files ?? [],
		session.selectedIndices,
		session.fileList ? formatFileSize(session.fileList.totalSize) : '--- MB',
		companionSummaryForInputIds,
	);
}

export function toInspectorViewFromInput(
	view: InputView,
	companionSummaryForInputIds: (inputIds: ReadonlyArray<string | undefined>) => {
		readonly text: string;
		readonly title: string;
	},
): InspectorView {
	return inspectorFromFiles(
		view.files,
		view.selectedIndices,
		view.hasFiles ? combinedSizeFromFiles(view.files) : '--- MB',
		companionSummaryForInputIds,
	);
}

function combinedSizeFromFiles(files: ReadonlyArray<AudioFile>): string {
	const totalSize = files.reduce(
		(sum, file) => sum + (file.isValid && file.size ? file.size : 0),
		0,
	);
	return formatFileSize(totalSize);
}

function inspectorFromFiles(
	files: ReadonlyArray<AudioFile>,
	selectedIndices: ReadonlyArray<number>,
	combinedSizeText: string,
	companionSummaryForInputIds: (inputIds: ReadonlyArray<string | undefined>) => {
		readonly text: string;
		readonly title: string;
	},
): InspectorView {
	const selectedFiles = selectedIndices
		.map((index) => files[index])
		.filter((file): file is AudioFile => Boolean(file));

	if (selectedFiles.length === 0) {
		return { ...EMPTY_INSPECTOR_VIEW, combinedSizeText };
	}

	if (selectedFiles.length === 1 && selectedFiles[0]) {
		const file = selectedFiles[0];
		const index = selectedIndices[0] ?? 0;
		const companion = companionSummaryForInputIds([file.inputId]);
		if (!file.isValid) {
			return {
				...EMPTY_INSPECTOR_VIEW,
				combinedSizeText,
				contextText: pathBasename(file.path, { fallback: 'path' }),
				contextVariant: 'single',
				contextDetail: `${index + 1} of ${files.length}`,
				companionsText: companion.text,
				companionsTitle: companion.title,
			};
		}
		return {
			contextText: pathBasename(file.path, { fallback: 'path' }),
			contextVariant: 'single',
			contextDetail: `${index + 1} of ${files.length}`,
			bitrateText: file.bitrate ? `${file.bitrate} kb/s` : 'N/A',
			sampleRateText: file.sampleRate ? `${file.sampleRate} Hz` : 'N/A',
			channelsText: file.channels ? `${file.channels} ch` : 'N/A',
			codecText: formatOptionalText(file.codecLabel),
			decoderText: formatOptionalText(file.selectedDecoder),
			fileSizeText: file.size ? formatFileSize(file.size) : 'N/A',
			companionsText: companion.text,
			companionsTitle: companion.title,
			combinedSizeText,
		};
	}

	const companion = companionSummaryForInputIds(selectedFiles.map((file) => file.inputId));
	return {
		contextText: `${selectedFiles.length} files selected`,
		contextVariant: 'multi',
		contextDetail: '',
		bitrateText: '---',
		sampleRateText: '---',
		channelsText: '---',
		codecText: summarizeSharedTextValue(selectedFiles, (file) => file.codecLabel),
		decoderText: summarizeSharedTextValue(selectedFiles, (file) => file.selectedDecoder),
		fileSizeText: '---',
		companionsText: companion.text,
		companionsTitle: companion.title,
		combinedSizeText,
	};
}

function formatOptionalText(value: string | undefined): string {
	if (typeof value !== 'string') return 'N/A';
	const trimmed = value.trim();
	return trimmed.length > 0 ? trimmed : 'N/A';
}

function summarizeSharedTextValue(
	files: ReadonlyArray<AudioFile>,
	pickValue: (file: AudioFile) => string | undefined,
): string {
	if (files.length === 0) return '---';
	const values = files.map((file) => {
		const value = pickValue(file);
		return typeof value === 'string' && value.trim().length > 0 ? value.trim() : null;
	});
	if (values.some((value) => value === null)) {
		return 'Mixed';
	}
	const unique = new Set(values);
	if (unique.size !== 1) {
		return 'Mixed';
	}
	return values[0] ?? 'Mixed';
}
