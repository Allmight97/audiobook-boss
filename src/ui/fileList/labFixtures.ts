import type { AudioFile, FileListInfo } from '../../types/audio';
import {
	resetImportOrder,
	setCurrentFileList,
	setOrderLocked,
	setSelectedFileIndices,
	setSelectedIndex,
} from './state.svelte';

export type FileListLabScenarioId = (typeof fileListLabScenarioIds)[number];

export const fileListLabScenarioIds = [
	'chapter-queue',
	'chapter-queue-locked',
	'empty-queue',
] as const;

export function isFileListLabScenarioId(value: string | null): value is FileListLabScenarioId {
	return (fileListLabScenarioIds as readonly string[]).includes(value ?? '');
}

const chapterFileNames = [
	'01 - The Final Empire - Chapter 01 - The Survivor of Hathsin.mp3',
	'02 - The Final Empire - Chapter 02 - A Thief in the Night.mp3',
	'03 - The Final Empire - Chapter 03 - The Skaa Rebellion Considers.mp3',
	'04 - The Final Empire - Chapter 04 - Luthadel by Ashfall.mp3',
	'05 - The Final Empire - Chapter 05 - Allomancy for Beginners.mp3',
	'06 - The Final Empire - Chapter 06 - House War Preparations.mp3',
	'07 - The Final Empire - Chapter 07 - The Ball at Keep Venture.mp3',
	'08 - The Final Empire - Chapter 08 - Mists over the City.mp3',
	'09 - The Final Empire - Chapter 09 - The Pits Remembered.mp3',
	'10 - The Final Empire - Chapter 10 - Kredik Shaw at Midnight.mp3',
	'11 - The Final Empire - Chapter 11 - The Eleventh Metal.mp3',
	'12 - The Final Empire - Chapter 12 - Ascension.mp3',
];

function chapterFile(name: string, index: number): AudioFile {
	const invalid = index === 8;
	return {
		path: `/books/mistborn/${name}`,
		inputId: `lab-input-${index + 1}`,
		isValid: !invalid,
		duration: 1_620 + index * 47,
		size: 24_500_000 + index * 1_400_000,
		bitrate: 128,
		sampleRate: 44_100,
		channels: 2,
		format: 'MP3',
		error: invalid ? 'Unreadable audio stream' : undefined,
	};
}

function chapterQueueFileList(): FileListInfo {
	const files = chapterFileNames.map(chapterFile);
	const validFiles = files.filter((file) => file.isValid);
	return {
		files,
		selectedDecoders: files.map(() => null),
		totalDuration: validFiles.reduce((sum, file) => sum + (file.duration ?? 0), 0),
		totalSize: validFiles.reduce((sum, file) => sum + (file.size ?? 0), 0),
		validCount: validFiles.length,
		invalidCount: files.length - validFiles.length,
	};
}

/**
 * Dev-only design-lab adapter: seeds FileList state with deterministic
 * fixtures so lab.html can render the real island without the Tauri backend.
 * Not exported from the runtime Public API Strip.
 */
export function applyFileListLabScenario(id: FileListLabScenarioId): void {
	setOrderLocked(false);

	if (id === 'empty-queue') {
		resetImportOrder([]);
		setCurrentFileList(null);
		return;
	}

	const fileList = chapterQueueFileList();
	setCurrentFileList(fileList);
	resetImportOrder(fileList.files);
	setSelectedFileIndices([2, 3, 7]);
	setSelectedIndex(3);

	if (id === 'chapter-queue-locked') {
		setOrderLocked(true);
	}
}
