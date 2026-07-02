import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { AudioFile, FileListInfo } from '../../types/audio';
import {
	effectiveCoverForFile,
	firstValidFilePath,
	resolveCoverDisplayPath,
	resolveCoverOwnerPaths,
} from './coverOwner';

const metadataStateMocks = vi.hoisted(() => ({
	getMetadataForFile: vi.fn(),
	getMetadataIntentPatchForFile: vi.fn(),
}));

vi.mock('../metadataSession', () => ({
	getMetadataForFile: metadataStateMocks.getMetadataForFile,
	getMetadataIntentPatchForFile: metadataStateMocks.getMetadataIntentPatchForFile,
}));

function makeFile(path: string, isValid = true): AudioFile {
	return {
		path,
		isValid,
		inputId: path,
	} as AudioFile;
}

function makeFileList(files: AudioFile[]): FileListInfo {
	return {
		files,
		validCount: files.filter((file) => file.isValid).length,
		invalidCount: files.filter((file) => !file.isValid).length,
		totalDuration: 0,
		totalSize: 0,
	} as FileListInfo;
}

describe('coverOwner', () => {
	beforeEach(() => {
		metadataStateMocks.getMetadataForFile.mockReset();
		metadataStateMocks.getMetadataIntentPatchForFile.mockReset();
	});

	it('resolves merge owner to the first valid input file', () => {
		const fileList = makeFileList([makeFile('/books/a.m4b'), makeFile('/books/b.m4b')]);

		expect(resolveCoverOwnerPaths('merge', fileList, [makeFile('/books/b.m4b')])).toEqual([
			'/books/a.m4b',
		]);
	});

	it('resolves batch owner to a single selected valid file only', () => {
		const fileList = makeFileList([
			makeFile('/books/a.m4b'),
			makeFile('/books/b.m4b'),
			makeFile('/books/c.m4b', false),
		]);

		expect(
			resolveCoverOwnerPaths('batch', fileList, [
				makeFile('/books/b.m4b'),
				makeFile('/books/c.m4b', false),
			]),
		).toEqual(['/books/b.m4b']);
	});

	it('ignores batch multi-select cover ownership', () => {
		const fileList = makeFileList([makeFile('/books/a.m4b'), makeFile('/books/b.m4b')]);

		expect(
			resolveCoverOwnerPaths('batch', fileList, [
				makeFile('/books/a.m4b'),
				makeFile('/books/b.m4b'),
			]),
		).toEqual([]);
	});

	it('prefers intent patch cover art over stored metadata', () => {
		metadataStateMocks.getMetadataIntentPatchForFile.mockReturnValue({
			cover_art: { op: 'set', value: [9, 9, 9] },
		});
		metadataStateMocks.getMetadataForFile.mockReturnValue({ cover_art: [1, 2, 3] });

		expect(effectiveCoverForFile('/books/a.m4b')).toEqual([9, 9, 9]);
	});

	it('treats intent clear as no cover art', () => {
		metadataStateMocks.getMetadataIntentPatchForFile.mockReturnValue({
			cover_art: { op: 'clear' },
		});
		metadataStateMocks.getMetadataForFile.mockReturnValue({ cover_art: [1, 2, 3] });

		expect(effectiveCoverForFile('/books/a.m4b')).toBeNull();
	});

	it('returns null display path when batch multi-select covers differ', () => {
		const fileList = makeFileList([makeFile('/books/a.m4b'), makeFile('/books/b.m4b')]);
		metadataStateMocks.getMetadataIntentPatchForFile.mockImplementation((path: string) => {
			if (path === '/books/a.m4b') {
				return { cover_art: { op: 'set', value: [1] } };
			}
			return { cover_art: { op: 'set', value: [2] } };
		});
		metadataStateMocks.getMetadataForFile.mockReturnValue({});

		expect(
			resolveCoverDisplayPath('batch', fileList, [
				makeFile('/books/a.m4b'),
				makeFile('/books/b.m4b'),
			]),
		).toBeNull();
	});

	it('returns first selected path when batch multi-select covers match', () => {
		const fileList = makeFileList([makeFile('/books/a.m4b'), makeFile('/books/b.m4b')]);
		metadataStateMocks.getMetadataIntentPatchForFile.mockReturnValue({
			cover_art: { op: 'set', value: [1, 2, 3] },
		});
		metadataStateMocks.getMetadataForFile.mockReturnValue({});

		expect(
			resolveCoverDisplayPath('batch', fileList, [
				makeFile('/books/a.m4b'),
				makeFile('/books/b.m4b'),
			]),
		).toBe('/books/a.m4b');
	});

	it('finds first valid file path in list order', () => {
		const fileList = makeFileList([
			makeFile('/books/invalid.m4b', false),
			makeFile('/books/first-valid.m4b'),
		]);

		expect(firstValidFilePath(fileList)).toBe('/books/first-valid.m4b');
	});
});
