import { describe, expect, it } from 'vitest';
import type { AudioFile, FileListInfo } from '../../../types/audio';
import {
	findFilePathByCurrentFile,
	findFilePathByIndex,
	findFilePathByName,
} from '../services/fileLookup';

function makeFileList(paths: string[]): FileListInfo {
	const files: AudioFile[] = paths.map((path) => ({
		path,
		isValid: true,
	}));

	return {
		files,
		totalDuration: 0,
		totalSize: 0,
		validCount: files.length,
		invalidCount: 0,
	};
}

describe('file lookup helpers', () => {
	it('prefers exact current_file path matches before basename fallback', () => {
		const fileList = makeFileList([
			'/library/first/chapter-01.m4b',
			'/library/second/chapter-01.m4b',
		]);

		expect(findFilePathByCurrentFile(fileList, '/library/second/chapter-01.m4b (1/10)')).toBe(
			'/library/second/chapter-01.m4b',
		);
	});

	it('returns null when basename fallback is ambiguous', () => {
		const fileList = makeFileList([
			'/library/first/chapter-01.m4b',
			'/library/second/chapter-01.m4b',
		]);

		expect(findFilePathByName(fileList, 'chapter-01.m4b')).toBeNull();
		expect(findFilePathByCurrentFile(fileList, 'chapter-01.m4b (1/10)')).toBeNull();
	});

	it('still supports indexed lookup for queue-correlated progress', () => {
		const fileList = makeFileList(['/library/alpha.m4b', '/library/beta.m4b']);

		expect(findFilePathByIndex(fileList, 1)).toBe('/library/beta.m4b');
		expect(findFilePathByIndex(fileList, 2)).toBeNull();
	});
});
