import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, waitFor } from '@testing-library/svelte';
import MetadataLookupIsland from '../metadataLookup/MetadataLookupIsland.svelte';

vi.mock('../../lib/tauri/client', () => ({
	tauriClient: {
		searchOnlineMetadata: vi.fn(),
		loadCoverArtFromUrl: vi.fn(),
	},
}));

vi.mock('../fileList/state.svelte', () => ({
	getCurrentFileList: () => ({ files: [] }),
	getSelectedFileIndices: () => new Set<number>(),
	isOrderLocked: vi.fn(() => false),
	onOrderLockChange: vi.fn(() => () => undefined),
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
import { metadataLookupState } from '../metadataLookup/state.svelte';

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

	it('does not render provider cover URLs as image sources', async () => {
		initMetadataLookup();
		metadataLookupState.results = [
			{
				source: 'audnexus',
				sourceId: 'audnexus:private',
				title: 'Private Cover',
				authors: ['Author One'],
				narrators: ['Narrator One'],
				series: undefined,
				seriesPart: undefined,
				subseries: undefined,
				subseriesPart: undefined,
				description: 'Description',
				publishedDate: '2020-07',
				durationSeconds: 3600,
				audibleOnly: false,
				coverUrl: 'http://169.254.169.254/latest/meta-data/iam/security-credentials/',
			},
			{
				source: 'openlibrary',
				sourceId: 'openlibrary:loopback',
				title: 'Loopback Cover',
				authors: ['Author Two'],
				narrators: ['Narrator Two'],
				series: undefined,
				seriesPart: undefined,
				subseries: undefined,
				subseriesPart: undefined,
				description: 'Description',
				publishedDate: '2021-08',
				durationSeconds: 7200,
				audibleOnly: false,
				coverUrl: 'http://127.0.0.1:8080/private-cover.jpg',
			},
		];

		await waitFor(() => {
			expect(
				document.querySelectorAll('[data-testid="metadata-lookup-cover-available"]'),
			).toHaveLength(2);
		});
		expect(document.querySelectorAll('#metadata-lookup-results img')).toHaveLength(0);
		expect(document.querySelector('[src*="169.254.169.254"]')).toBeNull();
		expect(document.querySelector('[src*="127.0.0.1"]')).toBeNull();
	});
});
