import { describe, expect, it } from 'vitest';
import type { FileListInfo } from '../../../types/audio';
import { findFilePathByIndex } from '../services/fileLookup';

function makeFileList(paths: string[]): FileListInfo {
	return {
		files: paths.map((path) => ({ path, size: 1, duration: 1, isValid: true })),
		selectedDecoders: paths.map(() => null),
		totalDuration: paths.length,
		totalSize: paths.length,
		validCount: paths.length,
		invalidCount: 0,
	};
}

describe('statusPanel fileLookup', () => {
	it('findFilePathByIndex resolves a valid queue index', () => {
		const fileList = makeFileList(['/library/first.m4b', '/library/second.m4b']);
		expect(findFilePathByIndex(fileList, 1)).toBe('/library/second.m4b');
	});

	it('findFilePathByIndex rejects out-of-range and non-integer indices', () => {
		const fileList = makeFileList(['/library/first.m4b']);
		expect(findFilePathByIndex(fileList, -1)).toBeNull();
		expect(findFilePathByIndex(fileList, 1)).toBeNull();
		expect(findFilePathByIndex(fileList, 1.5)).toBeNull();
		expect(findFilePathByIndex(null, 0)).toBeNull();
	});
});
