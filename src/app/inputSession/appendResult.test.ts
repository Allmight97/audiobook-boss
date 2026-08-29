import { describe, expect, it } from 'vitest';
import type { AudioFile, FileListInfo } from '../../types/audio';
import {
	buildFileListAppendResult,
	collectUniqueFiles,
	normalizeFileListInfo,
} from './appendResult';

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

function fileList(
	files: AudioFile[],
	selectedDecoders: FileListInfo['selectedDecoders'] = files.map(() => null),
): FileListInfo {
	return {
		files,
		selectedDecoders,
		totalDuration: 0,
		totalSize: 0,
		validCount: 0,
		invalidCount: 0,
	};
}

describe('file list append result', () => {
	it('normalizes duplicate files and derived totals for a replacement list', () => {
		const alpha = file('/books/alpha.m4b', { duration: 12, size: 120 });
		const duplicateAlpha = file('/books/alpha.m4b', { duration: 99, size: 990 });
		const invalid = file('/books/broken.m4b', { isValid: false, duration: 20, size: 200 });

		const normalized = normalizeFileListInfo(fileList([alpha, duplicateAlpha, invalid]));

		expect(normalized.files.map((entry) => entry.path)).toEqual([
			'/books/alpha.m4b',
			'/books/broken.m4b',
		]);
		expect(normalized.validCount).toBe(1);
		expect(normalized.invalidCount).toBe(1);
		expect(normalized.totalDuration).toBe(12);
		expect(normalized.totalSize).toBe(120);
	});

	it('reports duplicate-only appends without creating a merged file list', () => {
		const alpha = file('/books/alpha.m4b');

		const result = buildFileListAppendResult(fileList([alpha]), {
			existingFiles: [alpha],
			currentFileList: fileList([alpha]),
		});

		expect(result.outcome).toBe('duplicateOnly');
		expect(result.fileList).toBeNull();
		expect(result.appendedFiles).toEqual([]);
		expect(result.incomingFiles.map((entry) => entry.path)).toEqual(['/books/alpha.m4b']);
	});

	it('appends only unseen files and preserves decoder selections by path', () => {
		const alpha = file('/books/alpha.m4b', { duration: 12, size: 120 });
		const beta = file('/books/beta.m4b', { duration: 30, size: 300 });
		const gamma = file('/books/gamma.m4b', { duration: 45, size: 450 });
		const appleDecoder = { decoderId: 'aac_at', decoderLabel: 'Apple AAC' };
		const ffmpegDecoder = { decoderId: 'ffmpeg', decoderLabel: 'FFmpeg AAC' };
		const current = fileList([alpha, beta], [appleDecoder, null]);
		const incoming = fileList([beta, gamma], [null, ffmpegDecoder]);

		const result = buildFileListAppendResult(incoming, {
			existingFiles: current.files,
			currentFileList: current,
		});

		expect(result.outcome).toBe('append');
		expect(result.appendedFiles.map((entry) => entry.path)).toEqual(['/books/gamma.m4b']);
		expect(result.fileList?.files.map((entry) => entry.path)).toEqual([
			'/books/alpha.m4b',
			'/books/beta.m4b',
			'/books/gamma.m4b',
		]);
		expect(result.fileList?.selectedDecoders).toEqual([appleDecoder, null, ffmpegDecoder]);
		expect(result.fileList?.totalDuration).toBe(87);
		expect(result.fileList?.totalSize).toBe(870);
	});

	it('does not mutate the caller-provided seen-path set', () => {
		const seenPaths = new Set(['/books/alpha.m4b']);

		const result = collectUniqueFiles(
			[file('/books/alpha.m4b'), file('/books/beta.m4b')],
			seenPaths,
		);

		expect(result.map((entry) => entry.path)).toEqual(['/books/beta.m4b']);
		expect(seenPaths).toEqual(new Set(['/books/alpha.m4b']));
	});
});
