import { beforeEach, describe, expect, it } from 'vitest';
import type { AudioFile, FileListInfo } from '../../../../types/audio';
import {
	cacheMetadataForFile,
	clearMetadataSession,
	stageMetadataIntentPatch,
} from '../../../metadataSession';
import { displayedArtistForFile, displayedTitleForFile } from '../view';
import { loadFileList, readFileListView, removeFile, reorderFiles, resetFileList } from '..';

function makeFile(path: string): AudioFile {
	return {
		path,
		inputId: path,
		isValid: true,
		duration: 60,
		size: 1024,
		format: 'm4b',
	};
}

function makeFileList(...files: AudioFile[]): FileListInfo {
	return {
		files,
		validCount: files.length,
		invalidCount: 0,
		totalDuration: files.reduce((sum, file) => sum + (file.duration || 0), 0),
		totalSize: files.reduce((sum, file) => sum + (file.size || 0), 0),
		selectedDecoders: files.map(() => null),
	};
}

describe('Solid 2 File List derived view', () => {
	beforeEach(() => {
		resetFileList();
		clearMetadataSession();
	});

	it('uses analyzed tags only until metadata session truth exists', () => {
		const tagged = {
			...makeFile('/books/tagged.m4b'),
			tagTitle: 'Analyzed Title',
			tagArtist: 'Analyzed Artist',
		};
		expect(displayedTitleForFile(tagged)).toBe('Analyzed Title');
		expect(displayedArtistForFile(tagged)).toBe('Analyzed Artist');

		cacheMetadataForFile(tagged.path, { title: 'Session Title', artist: 'Session Artist' });
		expect(displayedTitleForFile(tagged)).toBe('Session Title');
		expect(displayedArtistForFile(tagged)).toBe('Session Artist');

		stageMetadataIntentPatch(tagged.path, {
			title: { op: 'clear' },
			artist: { op: 'clear' },
		});
		expect(displayedTitleForFile(tagged)).toBe('tagged.m4b');
		expect(displayedArtistForFile(tagged)).toBeNull();
	});

	it('reflects session reorder without manual view sync', () => {
		loadFileList(
			makeFileList(
				makeFile('/books/alpha.m4b'),
				makeFile('/books/beta.m4b'),
				makeFile('/books/gamma.m4b'),
			),
		);
		reorderFiles(0, 2);
		expect(readFileListView().files.map((file) => file.path)).toEqual([
			'/books/beta.m4b',
			'/books/gamma.m4b',
			'/books/alpha.m4b',
		]);
	});

	it('reflects session remove without manual view sync', () => {
		loadFileList(makeFileList(makeFile('/books/alpha.m4b'), makeFile('/books/beta.m4b')));
		removeFile(0);
		const view = readFileListView();
		expect(view.files.map((file) => file.path)).toEqual(['/books/beta.m4b']);
		expect(view.selectedIndices).toEqual([]);
	});
});
