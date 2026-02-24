import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render } from '@testing-library/svelte';
import MetadataLookupIsland from '../metadataLookup/MetadataLookupIsland.svelte';

vi.mock('../../lib/tauri/client', () => ({
	tauriClient: {
		searchOnlineMetadata: vi.fn(),
		loadCoverArtFromUrl: vi.fn(),
	},
}));

vi.mock('../fileList', () => ({
	getCurrentFileList: () => ({ files: [] }),
}));

vi.mock('../fileList/state', () => ({
	getSelectedFileIndices: () => new Set<number>(),
}));

vi.mock('../fileList/actions', () => ({
	selectFile: vi.fn(),
}));

vi.mock('../metadataForm', () => ({
	applyMetadataToForm: vi.fn(),
	readMetadataForm: vi.fn(() => ({})),
}));

vi.mock('../outputPanel', () => ({
	updateEstimatedSize: vi.fn(),
	updateOutputPath: vi.fn(),
}));

vi.mock('../tagPreview', () => ({
	updateTagPreview: vi.fn(),
}));

vi.mock('../coverArt', () => ({
	clearCoverArt: vi.fn(),
	setCoverArt: vi.fn(),
	setCustomCoverArt: vi.fn(),
}));

vi.mock('../metadataState', () => ({
	getMetadataForFile: vi.fn(() => ({})),
	setMetadataForFile: vi.fn(),
}));

import { initMetadataLookup } from '../metadataLookup';

describe('MetadataLookup island mount', () => {
	beforeEach(() => {
		document.body.innerHTML = `
      <button id="metadata-lookup-btn">Open</button>
    `;
		render(MetadataLookupIsland);
	});

	it('mounts modal controls into root', () => {
		initMetadataLookup();

		expect(document.getElementById('metadata-lookup-modal')).toBeTruthy();
		expect(document.getElementById('metadata-lookup-search-btn')).toBeTruthy();
		expect(document.getElementById('metadata-lookup-skip-btn')).toBeTruthy();
	});
});
