import { describe, expect, it, vi } from 'vitest';
import { createCoverArtTracker } from '../services/coverArtTracker';

describe('coverArtTracker', () => {
	it('syncs the first valid title through the injected title reader', async () => {
		const readCoverArtDataUrl = vi.fn(async () => 'data:image/png;base64,current');
		const displayCoverArt = vi.fn();
		const resetArtThumbnail = vi.fn();
		const validTitles = vi.fn(() => [{ path: '/books/current.m4b', isValid: true }]);
		const tracker = createCoverArtTracker({
			validTitles,
			readCoverArtDataUrl,
			displayCoverArt,
			resetArtThumbnail,
		});

		await tracker.syncForCurrentList();

		expect(validTitles).toHaveBeenCalledTimes(1);
		expect(readCoverArtDataUrl).toHaveBeenCalledWith('/books/current.m4b');
		expect(displayCoverArt).toHaveBeenCalledWith('data:image/png;base64,current');
		expect(resetArtThumbnail).not.toHaveBeenCalled();
	});

	it('resets the thumbnail when there is no valid file to sync', async () => {
		const readCoverArtDataUrl = vi.fn(async () => 'data:image/png;base64,alpha');
		const resetArtThumbnail = vi.fn();
		const tracker = createCoverArtTracker({
			validTitles: () => [],
			readCoverArtDataUrl,
			displayCoverArt: vi.fn(),
			resetArtThumbnail,
		});

		await tracker.syncForCurrentList();

		expect(readCoverArtDataUrl).not.toHaveBeenCalled();
		expect(resetArtThumbnail).toHaveBeenCalledTimes(1);
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
