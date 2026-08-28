import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, waitFor } from '@testing-library/svelte';
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
	refreshCoverArtDisplayMock: vi.fn(),
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
	stageMetadataIntentPatchMock: vi.fn(),
	activeIndex: 0,
}));

vi.mock('../../lib/tauri/client', () => ({
	tauriClient: {
		searchOnlineMetadata: context.searchOnlineMetadataMock,
		loadCoverArtFromUrl: context.loadCoverArtFromUrlMock,
	},
}));

vi.mock('../fileList/state', () => ({
	getCurrentFileList: () => context.currentFileList,
	getSelectedFileIndices: () => context.selectedIndices,
	isOrderLocked: vi.fn(() => false),
	onOrderLockChange: vi.fn(() => () => undefined),
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
	refreshCoverArtDisplay: context.refreshCoverArtDisplayMock,
	setCoverArt: context.setCoverArtMock,
	setCustomCoverArt: context.setCustomCoverArtMock,
}));

vi.mock('../metadataSession', () => ({
	getMetadataForFile: (filePath: string) => context.metadataByFile.get(filePath),
	stageMetadataIntentPatch: context.stageMetadataIntentPatchMock,
	buildMetadataDraftIntent: (metadata: Record<string, unknown>) =>
		Object.fromEntries(
			Object.entries(metadata)
				.filter(([, value]) => value !== undefined)
				.map(([key, value]) => [key, { op: 'set', value }]),
		),
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
	return (document.getElementById('metadata-lookup-title-query') as HTMLInputElement).value;
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
	await waitFor(() => {
		expect(document.getElementById('metadata-lookup-modal')?.classList.contains('open')).toBe(true);
	});
}

async function runSearchAndApply(): Promise<void> {
	const applyButton = await waitFor(() => {
		const button = document.querySelector<HTMLButtonElement>(
			"#metadata-lookup-results button[data-index='0']",
		);
		if (!button) throw new Error('Expected an apply button');
		return button;
	});
	applyButton.click();
	await flushAsync();
}

async function waitForStatus(text: string): Promise<void> {
	await waitFor(() => {
		expect(getStatusText()).toBe(text);
	});
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
		context.stageMetadataIntentPatchMock.mockReset();
		context.stageMetadataIntentPatchMock.mockImplementation(
			(filePath: string, intentPatch: Record<string, { op: string; value?: unknown }>) => {
				const next: StoredMetadata = { ...(context.metadataByFile.get(filePath) ?? {}) };
				for (const [key, intent] of Object.entries(intentPatch)) {
					if (!intent || intent.op === 'noop') continue;
					if (intent.op === 'clear') {
						delete (next as Record<string, unknown>)[key];
						continue;
					}
					if (intent.op === 'set') {
						(next as Record<string, unknown>)[key] = intent.value;
					}
				}
				context.metadataByFile.set(filePath, next);
				return 'staged' as const;
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
		context.searchOnlineMetadataMock.mockResolvedValue({
			results: [
				{
					source: 'audnexus',
					sourceId: 'audnexus:1',
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
			],
			diagnostics: [],
		});
		context.loadCoverArtFromUrlMock.mockReset();
		context.loadCoverArtFromUrlMock.mockResolvedValue([9, 9, 9]);
	});

	it('does not wipe previously replaced art when queue advances', async () => {
		await initLookup();

		const toggle = document.getElementById('metadata-lookup-cover-toggle') as HTMLInputElement;
		toggle.checked = true;
		toggle.dispatchEvent(new Event('change'));

		await runSearchAndApply();

		await waitForStatus('Metadata applied. Found 1 results.');
		expect(getContextText()).toBe('2 of 2 • beta.m4b');
		expect(getQueryValue()).toBe('Beta Existing');
		expect(context.loadCoverArtFromUrlMock).toHaveBeenCalledWith('https://example.com/cover.jpg');
		expect(context.stageMetadataIntentPatchMock).toHaveBeenCalledWith(
			'/books/alpha.m4b',
			expect.objectContaining({
				title: { op: 'set', value: 'Alpha Patched' },
				album: { op: 'set', value: 'Alpha Patched' },
				cover_art: { op: 'set', value: [9, 9, 9] },
			}),
		);
		expect(context.metadataByFile.get('/books/alpha.m4b')).toEqual(
			expect.objectContaining({
				title: 'Alpha Patched',
				album: 'Alpha Patched',
				cover_art: [9, 9, 9],
			}),
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

		await waitForStatus('Metadata applied. Found 1 results.');
		expect(context.loadCoverArtFromUrlMock).toHaveBeenCalledWith('https://example.com/cover.jpg');
		expect(context.setCustomCoverArtMock).not.toHaveBeenCalled();
		expect(context.stageMetadataIntentPatchMock).toHaveBeenCalledWith(
			'/books/alpha.m4b',
			expect.not.objectContaining({ cover_art: expect.anything() }),
		);
		// Existing cover art survives untouched in the session cache.
		expect(context.metadataByFile.get('/books/alpha.m4b')).toEqual(
			expect.objectContaining({ cover_art: [1, 1, 1] }),
		);
	});

	it('applies mixed keep/replace decisions per file without bleed', async () => {
		await initLookup();

		const toggle = document.getElementById('metadata-lookup-cover-toggle') as HTMLInputElement;
		toggle.checked = true;
		toggle.dispatchEvent(new Event('change'));
		await runSearchAndApply();
		await waitForStatus('Metadata applied. Found 1 results.');

		toggle.checked = false;
		toggle.dispatchEvent(new Event('change'));
		await runSearchAndApply();
		await waitForStatus('Queue complete.');

		expect(context.metadataByFile.get('/books/alpha.m4b')).toEqual(
			expect.objectContaining({ cover_art: [9, 9, 9] }),
		);
		expect(context.metadataByFile.get('/books/beta.m4b')).toEqual(
			expect.objectContaining({ cover_art: [2, 2, 2] }),
		);
		expect(context.loadCoverArtFromUrlMock).toHaveBeenCalledTimes(2);
		expect(context.refreshCoverArtDisplayMock).toHaveBeenCalled();
	});

	it('does not mutate metadata when skipping queue item', async () => {
		await initLookup();

		click('metadata-lookup-skip-btn');
		await waitForStatus('Skipped. Found 1 results.');

		expect(context.stageMetadataIntentPatchMock).not.toHaveBeenCalled();
		expect(getContextText()).toBe('2 of 2 • beta.m4b');
		expect(context.selectFileMock).toHaveBeenCalledWith(
			1,
			{ multi: false, range: false },
			{ skipPersistPrevious: true },
		);
	});

	it('shows manual-entry CTA when search returns no results and focuses metadata title', async () => {
		context.searchOnlineMetadataMock.mockResolvedValueOnce({ results: [], diagnostics: [] });
		await initLookup();

		await waitFor(() => {
			expect(document.querySelector('.app-modal-empty')?.textContent ?? '').toContain(
				'Older CD-era or rare audiobook editions may not be indexed.',
			);
		});

		const emptyState = document.querySelector('.app-modal-empty');
		expect(emptyState?.textContent ?? '').toContain(
			'Older CD-era or rare audiobook editions may not be indexed.',
		);
		const manualButton = document.getElementById(
			'metadata-lookup-manual-entry-btn',
		) as HTMLButtonElement | null;
		expect(manualButton).toBeTruthy();
		manualButton?.click();

		await waitFor(() => {
			expect(document.getElementById('metadata-lookup-modal')?.classList.contains('open')).toBe(
				false,
			);
			expect((document.activeElement as HTMLElement | null)?.id).toBe('meta-title');
		});
	});

	it('keeps backend failure distinct from no-result state', async () => {
		context.searchOnlineMetadataMock.mockRejectedValueOnce(new Error('all sources failed'));
		await initLookup();

		await waitForStatus('Search failed. Check your query and try again.');

		expect(document.querySelector('.app-modal-empty')).toBeNull();
		expect(document.getElementById('metadata-lookup-manual-entry-btn')).toBeNull();
		expect(document.querySelector("#metadata-lookup-results button[data-index='0']")).toBeNull();
	});
});
