import { beforeEach, describe, expect, it } from 'vitest';
import type { AudioFile, FileListInfo } from '../../../types/audio';
import { removeFile, reorderFiles } from '../actions';
import { setCurrentFileList } from '../state';
import {
	displayedArtistForFile,
	displayedTitleForFile,
	readFileListSelectedIndices,
	readFileListViewFiles,
} from '../viewState';
import {
	cacheMetadataForFile,
	clearMetadataSession,
	stageMetadataIntentPatch,
} from '../../metadataSession';

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

describe('derived file list view accessors', () => {
	beforeEach(() => {
		setCurrentFileList(null);
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

	it('keeps analyzed tags visible through a cover-only cache without weakening explicit clears', () => {
		const tagged = {
			...makeFile('/books/cover-only.m4b'),
			tagTitle: 'Analyzed Title',
			tagArtist: 'Analyzed Artist',
		};

		cacheMetadataForFile(tagged.path, { cover_art: [1, 2, 3] });
		expect(displayedTitleForFile(tagged)).toBe('Analyzed Title');
		expect(displayedArtistForFile(tagged)).toBe('Analyzed Artist');

		cacheMetadataForFile(tagged.path, {
			title: 'Loaded Title',
			artist: 'Loaded Artist',
			cover_art: [1, 2, 3],
		});
		stageMetadataIntentPatch(tagged.path, {
			title: { op: 'clear' },
			artist: { op: 'clear' },
		});
		expect(displayedTitleForFile(tagged)).toBe('cover-only.m4b');
		expect(displayedArtistForFile(tagged)).toBeNull();
	});

	it('reflects session reorder without manual view sync', () => {
		const alpha = makeFile('/books/alpha.m4b');
		const beta = makeFile('/books/beta.m4b');
		const gamma = makeFile('/books/gamma.m4b');
		setCurrentFileList(makeFileList(alpha, beta, gamma));

		reorderFiles(0, 2);

		expect(readFileListViewFiles().map((file) => file.path)).toEqual([
			'/books/beta.m4b',
			'/books/gamma.m4b',
			'/books/alpha.m4b',
		]);
	});

	it('reflects session remove without manual view sync', async () => {
		const alpha = makeFile('/books/alpha.m4b');
		const beta = makeFile('/books/beta.m4b');
		setCurrentFileList(makeFileList(alpha, beta));

		await removeFile(0);

		expect(readFileListViewFiles().map((file) => file.path)).toEqual(['/books/beta.m4b']);
		expect(readFileListSelectedIndices()).toEqual([]);
	});
});
