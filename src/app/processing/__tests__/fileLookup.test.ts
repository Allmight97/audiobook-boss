import { describe, expect, it } from 'vitest';
import type { AudioFile } from '../../../types/audio';
import { findFilePathByCurrentFile, findFilePathByIndex } from '../services/fileLookup';

function titles(paths: string[]): AudioFile[] {
	return paths.map((path) => ({ path, isValid: true }));
}

describe('file lookup helpers', () => {
	it('prefers exact current_file path matches before basename fallback', () => {
		const list = titles(['/library/first/chapter-01.m4b', '/library/second/chapter-01.m4b']);

		expect(findFilePathByCurrentFile(list, '/library/second/chapter-01.m4b (1/10)')).toBe(
			'/library/second/chapter-01.m4b',
		);
	});

	it('returns null when basename fallback is ambiguous', () => {
		const list = titles(['/library/first/chapter-01.m4b', '/library/second/chapter-01.m4b']);

		expect(findFilePathByCurrentFile(list, 'chapter-01.m4b (1/10)')).toBeNull();
	});

	it('still supports indexed lookup for queue-correlated progress', () => {
		const list = titles(['/library/alpha.m4b', '/library/beta.m4b']);

		expect(findFilePathByIndex(list, 1)).toBe('/library/beta.m4b');
		expect(findFilePathByIndex(list, 2)).toBeNull();
	});
});
