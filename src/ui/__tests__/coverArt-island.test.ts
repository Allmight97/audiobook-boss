import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render } from '@testing-library/svelte';
import CoverArtIsland from '../coverArt/CoverArtIsland.svelte';
import {
	getCurrentCoverArt,
	isCoverArtRemovalRequested,
	onClearCoverArt,
	onLoadCoverArtFromFilePicker,
	onLoadCoverArtFromInput,
	setCoverArt,
} from '../coverArt';

vi.mock('../../lib/tauri/client', () => ({
	tauriClient: {
		listen: vi.fn(),
		open: vi.fn(),
		loadCoverArtFile: vi.fn(),
		loadCoverArtFromUrl: vi.fn(),
	},
}));

describe('CoverArt island mount + clear behavior', () => {
	beforeEach(() => {
		render(CoverArtIsland, {
			onLoadFromFile: onLoadCoverArtFromFilePicker,
			onLoadFromInput: onLoadCoverArtFromInput,
			onClearCoverArt,
		});
	});

	it('mounts from root and clears cover art through UI action', () => {
		setCoverArt([0x89, 0x50, 0x4e, 0x47]);

		const clearButton = document.getElementById('cover-art-clear-btn') as HTMLButtonElement | null;
		expect(clearButton).toBeTruthy();
		expect(getCurrentCoverArt()).toEqual([0x89, 0x50, 0x4e, 0x47]);

		clearButton?.click();

		expect(getCurrentCoverArt()).toBeNull();
		expect(isCoverArtRemovalRequested()).toBe(true);
	});
});
