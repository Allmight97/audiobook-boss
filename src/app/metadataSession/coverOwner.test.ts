import { describe, expect, it } from 'vitest';
import type { AudioFile, FileListInfo } from '../../types/audio';
import { createMetadataCache } from './cache';
import {
	effectiveCoverForFile,
	firstValidFilePath,
	resolveCoverDisplayPath,
	resolveCoverOwnerPaths,
} from './coverOwner';

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
		const cache = createMetadataCache();
		cache.cacheMetadataForFile('/books/a.m4b', { cover_art: [1, 2, 3] });
		cache.stageMetadataIntentPatch('/books/a.m4b', {
			cover_art: { op: 'set', value: [9, 9, 9] },
		});

		expect(effectiveCoverForFile('/books/a.m4b', cache)).toEqual([9, 9, 9]);
	});

	it('treats intent clear as no cover art', () => {
		const cache = createMetadataCache();
		cache.cacheMetadataForFile('/books/a.m4b', { cover_art: [1, 2, 3] });
		cache.stageMetadataIntentPatch('/books/a.m4b', { cover_art: { op: 'clear' } });

		expect(effectiveCoverForFile('/books/a.m4b', cache)).toBeNull();
	});

	it('returns null display path when batch multi-select covers differ', () => {
		const cache = createMetadataCache();
		const fileList = makeFileList([makeFile('/books/a.m4b'), makeFile('/books/b.m4b')]);
		cache.stageMetadataIntentPatch('/books/a.m4b', { cover_art: { op: 'set', value: [1] } });
		cache.stageMetadataIntentPatch('/books/b.m4b', { cover_art: { op: 'set', value: [2] } });

		expect(
			resolveCoverDisplayPath(
				'batch',
				fileList,
				[makeFile('/books/a.m4b'), makeFile('/books/b.m4b')],
				cache,
			),
		).toBeNull();
	});

	it('returns first selected path when batch multi-select covers match', () => {
		const cache = createMetadataCache();
		const fileList = makeFileList([makeFile('/books/a.m4b'), makeFile('/books/b.m4b')]);
		cache.stageMetadataIntentPatch('/books/a.m4b', {
			cover_art: { op: 'set', value: [1, 2, 3] },
		});
		cache.stageMetadataIntentPatch('/books/b.m4b', {
			cover_art: { op: 'set', value: [1, 2, 3] },
		});

		expect(
			resolveCoverDisplayPath(
				'batch',
				fileList,
				[makeFile('/books/a.m4b'), makeFile('/books/b.m4b')],
				cache,
			),
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
