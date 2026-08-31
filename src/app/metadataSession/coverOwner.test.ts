import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { AudioFile, FileListInfo } from '../../types/audio';
import {
	effectiveCoverForFile,
	firstValidFilePath,
	resolveCoverDisplayPath,
	resolveCoverOwnerPaths,
} from './coverOwner';

const metadataSessionMocks = vi.hoisted(() => ({
	getMetadataForFile: vi.fn(),
	getMetadataIntentPatchForFile: vi.fn(),
	getCoverDisplayForFile: vi.fn(),
}));

vi.mock('./cache', () => ({
	getMetadataForFile: metadataSessionMocks.getMetadataForFile,
	getMetadataIntentPatchForFile: metadataSessionMocks.getMetadataIntentPatchForFile,
	getCoverDisplayForFile: metadataSessionMocks.getCoverDisplayForFile,
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
		metadataSessionMocks.getMetadataForFile.mockReset();
		metadataSessionMocks.getMetadataIntentPatchForFile.mockReset();
		metadataSessionMocks.getCoverDisplayForFile.mockReset();
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

	it('prefers a staged cover handle over an embedded display cache', () => {
		metadataSessionMocks.getMetadataIntentPatchForFile.mockReturnValue({
			cover_art: { op: 'set', value: 'cover-9' },
		});
		metadataSessionMocks.getCoverDisplayForFile.mockReturnValue({
			status: 'staged',
			handleId: 'cover-9',
			dataUrl: 'data:image/jpeg;base64,abc',
		});

		expect(effectiveCoverForFile('/books/a.m4b')).toEqual({
			status: 'staged',
			handleId: 'cover-9',
			dataUrl: 'data:image/jpeg;base64,abc',
		});
	});

	it('treats intent clear as cleared cover art', () => {
		metadataSessionMocks.getMetadataIntentPatchForFile.mockReturnValue({
			cover_art: { op: 'clear' },
		});
		metadataSessionMocks.getCoverDisplayForFile.mockReturnValue({
			status: 'embedded',
			dataUrl: 'data:image/jpeg;base64,abc',
		});

		expect(effectiveCoverForFile('/books/a.m4b')).toEqual({ status: 'cleared' });
	});

	it('returns null display path when batch multi-select covers differ', () => {
		const fileList = makeFileList([makeFile('/books/a.m4b'), makeFile('/books/b.m4b')]);
		metadataSessionMocks.getMetadataIntentPatchForFile.mockImplementation((path: string) => {
			if (path === '/books/a.m4b') {
				return { cover_art: { op: 'set', value: 'cover-a' } };
			}
			return { cover_art: { op: 'set', value: 'cover-b' } };
		});
		metadataSessionMocks.getMetadataForFile.mockReturnValue({});

		expect(
			resolveCoverDisplayPath('batch', fileList, [
				makeFile('/books/a.m4b'),
				makeFile('/books/b.m4b'),
			]),
		).toBeNull();
	});

	it('returns first selected path when batch multi-select covers match', () => {
		const fileList = makeFileList([makeFile('/books/a.m4b'), makeFile('/books/b.m4b')]);
		metadataSessionMocks.getMetadataIntentPatchForFile.mockReturnValue({
			cover_art: { op: 'set', value: 'cover-shared' },
		});
		metadataSessionMocks.getMetadataForFile.mockReturnValue({});

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
