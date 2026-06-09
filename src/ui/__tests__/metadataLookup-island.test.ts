import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import MetadataLookupIsland from '../metadataLookup/MetadataLookupIsland.svelte';

const { loadCoverArtFromUrlMock } = vi.hoisted(() => ({
	loadCoverArtFromUrlMock: vi.fn(),
}));

vi.mock('../../lib/tauri/client', () => ({
	tauriClient: {
		searchOnlineMetadata: vi.fn(),
		loadCoverArtFromUrl: loadCoverArtFromUrlMock,
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
	refreshCoverArtDisplay: vi.fn(),
	coverArtBytesToDataUrl: (bytes: number[]) =>
		`data:image/jpeg;base64,${btoa(String.fromCharCode(...bytes))}`,
}));

vi.mock('../metadataState', () => ({
	getMetadataForFile: vi.fn(() => ({})),
	setMetadataForFile: vi.fn(),
}));

import { tick } from 'svelte';
import { initMetadataLookup } from '../metadataLookup';
import { metadataLookupState } from '../metadataLookup/state.svelte';
import { clearMetadataLookupCoverPreviewCache } from '../metadataLookup/metadataLookupCoverPreview.svelte';

describe('MetadataLookup island mount', () => {
	beforeEach(() => {
		clearMetadataLookupCoverPreviewCache();
		loadCoverArtFromUrlMock.mockReset();
		loadCoverArtFromUrlMock.mockResolvedValue([0xff, 0xd8, 0xff]);
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

	it('loads cover previews through the backend without exposing provider URLs', async () => {
		initMetadataLookup();
		metadataLookupState.isOpen = true;
		metadataLookupState.hasSearched = true;
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
				coverUrl: 'https://covers.example.com/private-cover.jpg',
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
				coverUrl: 'https://covers.example.com/loopback-cover.jpg',
			},
		];

		await tick();

		const coverAreas = document.querySelectorAll('.metadata-lookup-cover');
		expect(coverAreas.length).toBe(2);

		await userEvent.hover(coverAreas[0] as Element);

		await waitFor(() => {
			expect(loadCoverArtFromUrlMock).toHaveBeenCalledWith(
				'https://covers.example.com/private-cover.jpg',
			);
		});

		await waitFor(() => {
			expect(document.querySelector('[data-testid="metadata-lookup-cover-image"]')).toBeTruthy();
		});

		const image = document.querySelector(
			'[data-testid="metadata-lookup-cover-image"]',
		) as HTMLImageElement | null;
		expect(image?.src.startsWith('data:image/jpeg;base64,')).toBe(true);
		expect(image?.src).not.toContain('covers.example.com');
		expect(document.querySelector('[src*="169.254.169.254"]')).toBeNull();
		expect(document.querySelector('[src*="127.0.0.1"]')).toBeNull();
	});
});
