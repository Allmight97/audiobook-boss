import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render } from '@testing-library/svelte';
import CoverArtIsland from '../coverArt/CoverArtIsland.svelte';
import {
	applyCoverArtDrop,
	COVER_ART_IMAGE_EXTENSION_HINTS,
	onClearCoverArt,
	onLoadCoverArtFromFilePicker,
	onLoadCoverArtFromInput,
	setCustomCoverArt,
	getCurrentCoverArt,
	isCoverArtRemovalRequested,
} from '../coverArt';
import { setCurrentFileList, setSelectedFileIndices, setSelectedIndex } from '../fileList';
import type { AudioFile, FileListInfo } from '../../types/audio';

const { openFileMock, loadCoverArtFileMock, loadCoverArtFromUrlMock } = vi.hoisted(() => ({
	openFileMock: vi.fn(),
	loadCoverArtFileMock: vi.fn(),
	loadCoverArtFromUrlMock: vi.fn(),
}));

vi.mock('../../lib/tauri/client', () => ({
	tauriClient: {
		listen: vi.fn(),
		openFile: openFileMock,
		loadCoverArtFile: loadCoverArtFileMock,
		loadCoverArtFromUrl: loadCoverArtFromUrlMock,
	},
}));

describe('CoverArt island mount + clear behavior', () => {
	beforeEach(() => {
		openFileMock.mockReset();
		loadCoverArtFileMock.mockReset();
		loadCoverArtFileMock.mockResolvedValue([0xff, 0xd8]);
		loadCoverArtFromUrlMock.mockReset();
		render(CoverArtIsland, {
			onLoadFromFile: onLoadCoverArtFromFilePicker,
			onLoadFromInput: onLoadCoverArtFromInput,
			onClearCoverArt,
		});
	});

	it('mounts from root and clears cover art through UI action', () => {
		setCurrentFileList({
			files: [{ path: '/books/a.m4b', isValid: true, inputId: 'a' } as AudioFile],
			validCount: 1,
			invalidCount: 0,
			totalDuration: 0,
			totalSize: 0,
		} as FileListInfo);
		setSelectedFileIndices([0]);
		setSelectedIndex(0);

		setCustomCoverArt([0x89, 0x50, 0x4e, 0x47]);

		const clearButton = document.getElementById('cover-art-clear-btn') as HTMLButtonElement | null;
		expect(clearButton).toBeTruthy();
		expect(getCurrentCoverArt()).toEqual([0x89, 0x50, 0x4e, 0x47]);

		clearButton?.click();

		expect(getCurrentCoverArt()).toBeNull();
		expect(isCoverArtRemovalRequested()).toBe(true);
	});

	it('uses centralized cover-art extension hints for the file picker', async () => {
		openFileMock.mockResolvedValue(null);

		await onLoadCoverArtFromFilePicker();

		expect(openFileMock).toHaveBeenCalledWith({
			title: 'Select Cover Art Image',
			filters: [
				{
					name: 'Image Files',
					extensions: [...COVER_ART_IMAGE_EXTENSION_HINTS],
				},
			],
		});
	});

	it('treats drag/drop extension matching as a frontend hint before backend validation', async () => {
		await expect(applyCoverArtDrop(['/tmp/cover.WEBP'])).resolves.toBe(true);
		await expect(applyCoverArtDrop(['/tmp/cover.gif'])).resolves.toBe(false);

		expect(loadCoverArtFileMock).toHaveBeenCalledTimes(1);
		expect(loadCoverArtFileMock).toHaveBeenCalledWith('/tmp/cover.WEBP');
	});

	it('keeps HTTPS precheck as a frontend hint before backend URL validation', async () => {
		await expect(onLoadCoverArtFromInput('http://example.com/cover.jpg')).resolves.toBeNull();

		expect(loadCoverArtFromUrlMock).not.toHaveBeenCalled();
		expect(document.body.textContent).toContain('Only HTTPS URLs are supported.');
	});

	it('shows sanitized backend cover-art validation errors', async () => {
		openFileMock.mockResolvedValue('/Users/example/private/cover.gif');
		loadCoverArtFileMock.mockRejectedValue({
			code: 'invalid_input',
			category: 'validation',
			message: 'Unsupported image format: gif. Supported formats: jpg, jpeg, png, webp',
			detail: '/Users/example/private/cover.gif',
		});

		await onLoadCoverArtFromFilePicker();

		expect(document.body.textContent).toContain(
			'Unsupported image format. Use JPEG, PNG, or WebP.',
		);
		expect(document.body.textContent).not.toContain('/Users/example/private/cover.gif');
	});
});
