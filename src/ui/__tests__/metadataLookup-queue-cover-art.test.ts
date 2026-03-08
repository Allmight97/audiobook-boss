import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render } from '@testing-library/svelte';
import type { AudiobookMetadata } from '../../types/metadata';
import MetadataLookupIsland from '../metadataLookup/MetadataLookupIsland.svelte';

type StoredMetadata = Partial<AudiobookMetadata>;

const context = vi.hoisted(() => ({
	searchOnlineMetadataMock: vi.fn(),
	loadCoverArtFromUrlMock: vi.fn(),
	selectFileMock: vi.fn(),
	applyMetadataToFormMock: vi.fn(),
	setCustomCoverArtMock: vi.fn(),
	clearCoverArtMock: vi.fn(),
	setCoverArtMock: vi.fn(),
	updateOutputPathMock: vi.fn(),
	updateEstimatedSizeMock: vi.fn(),
	updateTagPreviewMock: vi.fn(),
	selectedIndices: new Set<number>([0, 1]),
	currentFileList: {
		files: [
			{ path: '/books/alpha.m4b', isValid: true },
			{ path: '/books/beta.m4b', isValid: true },
		],
	} as { files: Array<{ path: string; isValid: boolean }> },
	metadataByFile: new Map<string, StoredMetadata>(),
	setMetadataForFileMock: vi.fn(),
	activeIndex: 0,
}));

vi.mock('../../lib/tauri/client', () => ({
	tauriClient: {
		searchOnlineMetadata: context.searchOnlineMetadataMock,
		loadCoverArtFromUrl: context.loadCoverArtFromUrlMock,
	},
}));

vi.mock('../fileList', () => ({
	getCurrentFileList: () => context.currentFileList,
}));

vi.mock('../fileList/state', () => ({
	getSelectedFileIndices: () => context.selectedIndices,
}));

vi.mock('../fileList/actions', () => ({
	selectFile: context.selectFileMock,
}));

vi.mock('../metadataForm', () => ({
	applyMetadataToForm: context.applyMetadataToFormMock,
	readMetadataForm: ({
		mode,
		includeCoverArt,
	}: {
		mode?: string;
		includeCoverArt?: boolean;
	} = {}) => {
		if (mode === 'single' && includeCoverArt === false) {
			return context.activeIndex === 0
				? { title: 'Alpha Patched', album: 'Alpha Patched' }
				: { title: 'Beta Patched', album: 'Beta Patched' };
		}
		return {};
	},
}));

vi.mock('../coverArt', () => ({
	clearCoverArt: context.clearCoverArtMock,
	setCoverArt: context.setCoverArtMock,
	setCustomCoverArt: context.setCustomCoverArtMock,
}));

vi.mock('../metadataState', () => ({
	getMetadataForFile: (filePath: string) => context.metadataByFile.get(filePath),
	setMetadataForFile: context.setMetadataForFileMock,
}));

vi.mock('../outputPanel', () => ({
	updateOutputPath: context.updateOutputPathMock,
	updateEstimatedSize: context.updateEstimatedSizeMock,
}));

vi.mock('../tagPreview', () => ({
	updateTagPreview: context.updateTagPreviewMock,
}));

function setupDom(): void {
	document.body.innerHTML = `
	    <input id="meta-title" />
	  `;
	render(MetadataLookupIsland);
}

function click(id: string): void {
	const el = document.getElementById(id) as HTMLElement | null;
	if (!el) throw new Error(`Missing element: ${id}`);
	el.click();
}

function getStatusText(): string {
	return (document.getElementById('metadata-lookup-status') as HTMLElement).textContent ?? '';
}

function getContextText(): string {
	return (document.getElementById('metadata-lookup-context') as HTMLElement).textContent ?? '';
}

function getQueryValue(): string {
	return (document.getElementById('metadata-lookup-query') as HTMLInputElement).value;
}

async function flushAsync(): Promise<void> {
	await Promise.resolve();
	await Promise.resolve();
	await Promise.resolve();
	await Promise.resolve();
}

async function initLookup(): Promise<void> {
	const module = await import('../metadataLookup');
	module.initMetadataLookup();
	module.openMetadataLookup();
	await flushAsync();
}

async function runSearchAndApply(): Promise<void> {
	click('metadata-lookup-search-btn');
	await flushAsync();
	const applyButton = document.querySelector<HTMLButtonElement>(
		"#metadata-lookup-results button[data-index='0']",
	);
	if (!applyButton) throw new Error('Expected an apply button');
	applyButton.click();
	await flushAsync();
}

describe('metadata lookup queue cover art isolation', () => {
	beforeEach(() => {
		setupDom();

		context.selectedIndices = new Set<number>([0, 1]);
		context.currentFileList = {
			files: [
				{ path: '/books/alpha.m4b', isValid: true },
				{ path: '/books/beta.m4b', isValid: true },
			],
		};
		context.metadataByFile = new Map<string, StoredMetadata>([
			['/books/alpha.m4b', { title: 'Alpha Existing', cover_art: [1, 1, 1] }],
			['/books/beta.m4b', { title: 'Beta Existing', cover_art: [2, 2, 2] }],
		]);
		context.setMetadataForFileMock.mockReset();
		context.setMetadataForFileMock.mockImplementation(
			(filePath: string, metadata: StoredMetadata) => {
				context.metadataByFile.set(filePath, metadata);
			},
		);
		context.activeIndex = 0;

		context.selectFileMock.mockReset();
		context.selectFileMock.mockImplementation(async (index: number) => {
			context.activeIndex = index;
		});
		context.applyMetadataToFormMock.mockReset();
		context.setCustomCoverArtMock.mockReset();
		context.clearCoverArtMock.mockReset();
		context.setCoverArtMock.mockReset();
		context.updateOutputPathMock.mockReset();
		context.updateEstimatedSizeMock.mockReset();
		context.updateTagPreviewMock.mockReset();

		context.searchOnlineMetadataMock.mockReset();
		context.searchOnlineMetadataMock.mockResolvedValue([
			{
				title: 'Lookup Title',
				authors: ['Author One'],
				narrators: ['Narrator One'],
				series: null,
				seriesPart: null,
				subseries: null,
				subseriesPart: null,
				description: 'Description',
				publishedDate: '2020-07',
				durationSeconds: 3600,
				audibleOnly: false,
				coverUrl: 'https://example.com/cover.jpg',
			},
		]);
		context.loadCoverArtFromUrlMock.mockReset();
		context.loadCoverArtFromUrlMock.mockResolvedValue([9, 9, 9]);
	});

	it('does not wipe previously replaced art when queue advances', async () => {
		await initLookup();

		const toggle = document.getElementById('metadata-lookup-cover-toggle') as HTMLInputElement;
		toggle.checked = true;
		toggle.dispatchEvent(new Event('change'));

		await runSearchAndApply();

		expect(getContextText()).toBe('2 of 2 • beta.m4b');
		expect(getStatusText()).toBe('Metadata applied. Ready for next search.');
		expect(getQueryValue()).toBe('Beta Existing');
		expect(context.loadCoverArtFromUrlMock).toHaveBeenCalledWith('https://example.com/cover.jpg');
		expect(context.setMetadataForFileMock).toHaveBeenCalledWith(
			'/books/alpha.m4b',
			expect.objectContaining({
				title: 'Alpha Patched',
				album: 'Alpha Patched',
				cover_art: [9, 9, 9],
			}),
			expect.objectContaining({ markPending: true }),
		);
		expect(context.updateTagPreviewMock).toHaveBeenCalledTimes(1);

		// Secondary check: queue navigation uses non-persisting selection handoff.
		const firstPathWrites = context.selectFileMock.mock.calls.filter(([index]) => index === 1);
		expect(firstPathWrites.length).toBeGreaterThanOrEqual(1);
		expect(firstPathWrites[0]?.[2]).toEqual({ skipPersistPrevious: true });
	});

	it('preserves existing cover art when replace toggle is disabled', async () => {
		await initLookup();

		const toggle = document.getElementById('metadata-lookup-cover-toggle') as HTMLInputElement;
		toggle.checked = false;
		toggle.dispatchEvent(new Event('change'));

		await runSearchAndApply();

		expect(context.loadCoverArtFromUrlMock).not.toHaveBeenCalled();
		expect(context.setMetadataForFileMock).toHaveBeenCalledWith(
			'/books/alpha.m4b',
			expect.objectContaining({ cover_art: [1, 1, 1] }),
			expect.objectContaining({ markPending: true }),
		);
		expect(getStatusText()).toBe('Metadata applied. Ready for next search.');
	});

	it('applies mixed keep/replace decisions per file without bleed', async () => {
		await initLookup();

		const toggle = document.getElementById('metadata-lookup-cover-toggle') as HTMLInputElement;
		toggle.checked = true;
		toggle.dispatchEvent(new Event('change'));
		await runSearchAndApply();

		toggle.checked = false;
		toggle.dispatchEvent(new Event('change'));
		await runSearchAndApply();

		const writesByPath = new Map<string, StoredMetadata>();
		context.setMetadataForFileMock.mock.calls.forEach(([filePath, metadata]) => {
			writesByPath.set(filePath as string, metadata as StoredMetadata);
		});

		expect(writesByPath.get('/books/alpha.m4b')).toEqual(
			expect.objectContaining({ cover_art: [9, 9, 9] }),
		);
		expect(writesByPath.get('/books/beta.m4b')).toEqual(
			expect.objectContaining({ cover_art: [2, 2, 2] }),
		);
		expect(context.loadCoverArtFromUrlMock).toHaveBeenCalledTimes(1);
		expect(context.setCoverArtMock).toHaveBeenLastCalledWith([2, 2, 2]);
		expect(getStatusText()).toBe('Queue complete.');
	});

	it('does not mutate metadata when skipping queue item', async () => {
		await initLookup();

		click('metadata-lookup-skip-btn');
		await flushAsync();

		expect(context.setMetadataForFileMock).not.toHaveBeenCalled();
		expect(getStatusText()).toBe('Skipped. Ready for next search.');
		expect(getContextText()).toBe('2 of 2 • beta.m4b');
		expect(context.selectFileMock).toHaveBeenCalledWith(
			1,
			{ multi: false, range: false },
			{ skipPersistPrevious: true },
		);
	});

	it('shows manual-entry CTA when search returns no results and focuses metadata title', async () => {
		context.searchOnlineMetadataMock.mockResolvedValueOnce([]);
		await initLookup();

		click('metadata-lookup-search-btn');
		await flushAsync();

		const emptyState = document.querySelector('.metadata-lookup-empty');
		expect(emptyState?.textContent ?? '').toContain(
			'Older CD-era or rare audiobook editions may not be indexed.',
		);
		const manualButton = document.getElementById(
			'metadata-lookup-manual-entry-btn',
		) as HTMLButtonElement | null;
		expect(manualButton).toBeTruthy();
		manualButton?.click();
		await flushAsync();

		expect(document.getElementById('metadata-lookup-modal')?.classList.contains('open')).toBe(
			false,
		);
		expect((document.activeElement as HTMLElement | null)?.id).toBe('meta-title');
	});

	it('keeps backend failure distinct from no-result state', async () => {
		context.searchOnlineMetadataMock.mockRejectedValueOnce(new Error('all sources failed'));
		await initLookup();

		click('metadata-lookup-search-btn');
		await flushAsync();

		expect(getStatusText()).toBe('Search failed. Check your query and try again.');
		expect(document.querySelector('.metadata-lookup-empty')).toBeNull();
		expect(document.getElementById('metadata-lookup-manual-entry-btn')).toBeNull();
		expect(document.querySelector("#metadata-lookup-results button[data-index='0']")).toBeNull();
	});
});
