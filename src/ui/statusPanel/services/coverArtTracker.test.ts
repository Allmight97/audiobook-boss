import { describe, expect, it, vi } from 'vitest';
import type { AudioFile, FileListInfo } from '../../../types/audio';
import { createCoverArtTracker } from './coverArtTracker';

function makeFileList(files: Array<{ path: string; isValid: boolean }>): FileListInfo {
	const audioFiles: AudioFile[] = files.map((file) => ({
		path: file.path,
		isValid: file.isValid,
	}));

	return {
		files: audioFiles,
		selectedDecoders: audioFiles.map(() => null),
		totalDuration: 0,
		totalSize: 0,
		validCount: audioFiles.filter((file) => file.isValid).length,
		invalidCount: audioFiles.filter((file) => !file.isValid).length,
	};
}

describe('coverArtTracker', () => {
	it('syncs the current file list through the injected file-list reader', async () => {
		const readCoverArtDataUrl = vi.fn(async () => 'data:image/png;base64,current');
		const displayCoverArt = vi.fn();
		const resetArtThumbnail = vi.fn();
		const getCurrentFileList = vi.fn(() =>
			makeFileList([{ path: '/books/current.m4b', isValid: true }]),
		);
		const tracker = createCoverArtTracker({
			getCurrentFileList,
			readCoverArtDataUrl,
			displayCoverArt,
			resetArtThumbnail,
		});

		await tracker.syncForCurrentList();

		expect(getCurrentFileList).toHaveBeenCalledTimes(1);
		expect(readCoverArtDataUrl).toHaveBeenCalledWith('/books/current.m4b');
		expect(displayCoverArt).toHaveBeenCalledWith('data:image/png;base64,current');
		expect(resetArtThumbnail).not.toHaveBeenCalled();
	});

	it('syncs the first valid file from a file list', async () => {
		const readCoverArtDataUrl = vi.fn(async () => 'data:image/png;base64,alpha');
		const displayCoverArt = vi.fn();
		const resetArtThumbnail = vi.fn();
		const tracker = createCoverArtTracker({
			readCoverArtDataUrl,
			displayCoverArt,
			resetArtThumbnail,
		});

		await tracker.syncFromFileList(
			makeFileList([
				{ path: '/books/invalid.m4b', isValid: false },
				{ path: '/books/alpha.m4b', isValid: true },
			]),
		);

		expect(readCoverArtDataUrl).toHaveBeenCalledTimes(1);
		expect(readCoverArtDataUrl).toHaveBeenCalledWith('/books/alpha.m4b');
		expect(displayCoverArt).toHaveBeenCalledWith('data:image/png;base64,alpha');
		expect(resetArtThumbnail).not.toHaveBeenCalled();
	});

	it('resets the thumbnail when there is no valid file to sync', async () => {
		const readCoverArtDataUrl = vi.fn(async () => 'data:image/png;base64,alpha');
		const resetArtThumbnail = vi.fn();
		const tracker = createCoverArtTracker({
			readCoverArtDataUrl,
			displayCoverArt: vi.fn(),
			resetArtThumbnail,
		});

		await tracker.syncFromFileList(makeFileList([{ path: '/books/invalid.m4b', isValid: false }]));
		await tracker.syncFromFileList(null);

		expect(readCoverArtDataUrl).not.toHaveBeenCalled();
		expect(resetArtThumbnail).toHaveBeenCalledTimes(2);
	});

	it('does not re-read the same file path until reset', async () => {
		const readCoverArtDataUrl = vi.fn(async () => null);
		const displayCoverArt = vi.fn();
		const resetArtThumbnail = vi.fn();
		const tracker = createCoverArtTracker({
			readCoverArtDataUrl,
			displayCoverArt,
			resetArtThumbnail,
		});

		await tracker.syncForFile('/books/alpha.m4b');
		await tracker.syncForFile('/books/alpha.m4b');

		expect(readCoverArtDataUrl).toHaveBeenCalledTimes(1);
		expect(resetArtThumbnail).toHaveBeenCalledTimes(1);
		expect(displayCoverArt).not.toHaveBeenCalled();
	});

	it('resets and warns when reading cover art fails', async () => {
		const error = new Error('metadata read failed');
		const readCoverArtDataUrl = vi.fn(async () => {
			throw error;
		});
		const displayCoverArt = vi.fn();
		const resetArtThumbnail = vi.fn();
		const warn = vi.fn();
		const tracker = createCoverArtTracker({
			readCoverArtDataUrl,
			displayCoverArt,
			resetArtThumbnail,
			warn,
		});

		await tracker.syncForFile('/books/alpha.m4b');

		expect(readCoverArtDataUrl).toHaveBeenCalledWith('/books/alpha.m4b');
		expect(displayCoverArt).not.toHaveBeenCalled();
		expect(resetArtThumbnail).toHaveBeenCalledTimes(1);
		expect(warn).toHaveBeenCalledWith('Failed to load cover art for thumbnail:', error);
	});

	it('clears tracked state on reset so the same file can be synced again', async () => {
		const readCoverArtDataUrl = vi.fn(async () => 'data:image/png;base64,alpha');
		const displayCoverArt = vi.fn();
		const resetArtThumbnail = vi.fn();
		const tracker = createCoverArtTracker({
			readCoverArtDataUrl,
			displayCoverArt,
			resetArtThumbnail,
		});

		await tracker.syncForFile('/books/alpha.m4b');
		tracker.reset();
		await tracker.syncForFile('/books/alpha.m4b');

		expect(readCoverArtDataUrl).toHaveBeenCalledTimes(2);
		expect(displayCoverArt).toHaveBeenCalledTimes(2);
		expect(resetArtThumbnail).toHaveBeenCalledTimes(1);
	});
});
