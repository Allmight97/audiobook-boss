import { describe, expect, it } from 'vitest';
import type { AudioFile } from '../../types/audio';
import { buildFileListAppendResult } from './appendResult';

function file(path: string, options: Partial<AudioFile> = {}): AudioFile {
	return {
		path,
		isValid: true,
		duration: 10,
		size: 100,
		bitrate: 64,
		sampleRate: 44_100,
		channels: 2,
		...options,
	};
}

describe('file list append result', () => {
	it('drops duplicate paths from a replacement list', () => {
		const alpha = file('/books/alpha.m4b', { duration: 12 });
		const duplicateAlpha = file('/books/alpha.m4b', { duration: 99 });
		const invalid = file('/books/broken.m4b', { isValid: false });

		const result = buildFileListAppendResult([alpha, duplicateAlpha, invalid], []);

		expect(result.outcome).toBe('replace');
		if (result.outcome === 'duplicateOnly') return;
		expect(result.files).toEqual([alpha, invalid]);
	});

	it('reports duplicate-only appends without creating a merged file list', () => {
		const alpha = file('/books/alpha.m4b');

		expect(buildFileListAppendResult([alpha], [alpha])).toEqual({ outcome: 'duplicateOnly' });
	});

	it('appends only unseen files after the existing order', () => {
		const alpha = file('/books/alpha.m4b');
		const beta = file('/books/beta.m4b');
		const gamma = file('/books/gamma.m4b');

		const result = buildFileListAppendResult([beta, gamma], [alpha, beta]);

		expect(result.outcome).toBe('append');
		if (result.outcome === 'duplicateOnly') return;
		expect(result.appendedFiles).toEqual([gamma]);
		expect(result.files).toEqual([alpha, beta, gamma]);
	});
});
