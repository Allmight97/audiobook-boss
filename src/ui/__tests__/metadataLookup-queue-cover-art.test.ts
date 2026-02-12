import { beforeEach, describe, expect, it, vi } from 'vitest';

const context = vi.hoisted(() => ({
	searchOnlineMetadataMock: vi.fn(),
	loadCoverArtFromUrlMock: vi.fn(),
	selectFileMock: vi.fn(),
	applyMetadataToFormMock: vi.fn(),
	setCustomCoverArtMock: vi.fn(),
	clearCoverArtMock: vi.fn(),
	setCoverArtMock: vi.fn(),
	onMetadataChangeMock: vi.fn(),
	updateTagPreviewMock: vi.fn(),
	selectedIndices: new Set<number>([0, 1]),
	currentFileList: {
		files: [
			{ path: '/books/alpha.m4b', isValid: true },
			{ path: '/books/beta.m4b', isValid: true },
		],
	} as { files: Array<{ path: string; isValid: boolean }> },
	metadataByFile: new Map<string, any>(),
	setMetadataForFileMock: vi.fn(),
	activeIndex: 0,
}));

vi.mock('../../lib/bridge', () => ({
	bridge: {
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
	onMetadataChange: context.onMetadataChangeMock,
}));

vi.mock('../tagPreview', () => ({
	updateTagPreview: context.updateTagPreviewMock,
}));

function setupDom(): void {
	document.body.innerHTML = `
    <button id="metadata-lookup-btn">Open</button>
    <button id="metadata-lookup-close">Close</button>
    <button id="metadata-lookup-search-btn">Search</button>
    <button id="metadata-lookup-skip-btn">Skip</button>
    <input id="metadata-lookup-query" />
    <select id="metadata-lookup-source">
      <option value="audnexus">audnexus</option>
    </select>
    <select id="metadata-lookup-apply-mode"></select>
    <input id="metadata-lookup-cover-toggle" type="checkbox" />
    <div id="metadata-lookup-status"></div>
    <div id="metadata-lookup-context"></div>
    <div id="metadata-lookup-results"></div>
    <div id="metadata-lookup-modal"></div>
  `;
}

function click(id: string): void {
	const el = document.getElementById(id) as HTMLElement | null;
	if (!el) throw new Error(`Missing element: ${id}`);
	el.click();
}

async function flushAsync(): Promise<void> {
	await Promise.resolve();
	await Promise.resolve();
}

async function initLookup(): Promise<void> {
	const module = await import('../metadataLookup');
	module.initMetadataLookup();
	click('metadata-lookup-btn');
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
		vi.resetModules();
		setupDom();

		context.selectedIndices = new Set<number>([0, 1]);
		context.currentFileList = {
			files: [
				{ path: '/books/alpha.m4b', isValid: true },
				{ path: '/books/beta.m4b', isValid: true },
			],
		};
		context.metadataByFile = new Map<string, any>([
			['/books/alpha.m4b', { title: 'Alpha Existing', cover_art: [1, 1, 1] }],
			['/books/beta.m4b', { title: 'Beta Existing', cover_art: [2, 2, 2] }],
		]);
		context.setMetadataForFileMock.mockReset();
		context.setMetadataForFileMock.mockImplementation((filePath: string, metadata: any) => {
			context.metadataByFile.set(filePath, metadata);
		});
		context.activeIndex = 0;

		context.selectFileMock.mockReset();
		context.selectFileMock.mockImplementation(async (index: number) => {
			context.activeIndex = index;
		});
		context.applyMetadataToFormMock.mockReset();
		context.setCustomCoverArtMock.mockReset();
		context.clearCoverArtMock.mockReset();
		context.setCoverArtMock.mockReset();
		context.onMetadataChangeMock.mockReset();
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
				publishedYear: 2020,
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

		await runSearchAndApply();

		const first = context.metadataByFile.get('/books/alpha.m4b');
		expect(first.cover_art).toEqual([9, 9, 9]);

		const firstPathWrites = context.selectFileMock.mock.calls.filter(([index]) => index === 1);
		expect(firstPathWrites.length).toBeGreaterThanOrEqual(1);
		expect(firstPathWrites[0]?.[2]).toEqual({ skipPersistPrevious: true });
		expect(context.setMetadataForFileMock).toHaveBeenCalledWith(
			'/books/alpha.m4b',
			expect.objectContaining({
				title: 'Alpha Patched',
				album: 'Alpha Patched',
				cover_art: [9, 9, 9],
			}),
			{ markPending: true },
		);
		expect(context.updateTagPreviewMock).toHaveBeenCalledTimes(1);
	});

	it('preserves existing cover art when replace toggle is disabled', async () => {
		await initLookup();

		const toggle = document.getElementById('metadata-lookup-cover-toggle') as HTMLInputElement;
		toggle.checked = false;

		await runSearchAndApply();

		const first = context.metadataByFile.get('/books/alpha.m4b');
		expect(first.cover_art).toEqual([1, 1, 1]);
	});

	it('applies mixed keep/replace decisions per file without bleed', async () => {
		await initLookup();

		const toggle = document.getElementById('metadata-lookup-cover-toggle') as HTMLInputElement;
		toggle.checked = true;
		await runSearchAndApply();

		toggle.checked = false;
		await runSearchAndApply();

		const first = context.metadataByFile.get('/books/alpha.m4b');
		const second = context.metadataByFile.get('/books/beta.m4b');
		expect(first.cover_art).toEqual([9, 9, 9]);
		expect(second.cover_art).toEqual([2, 2, 2]);
		const pendingCalls = context.setMetadataForFileMock.mock.calls.map((call) => call[2]);
		expect(pendingCalls).toEqual(
			expect.arrayContaining([{ markPending: true }, { markPending: true }]),
		);
	});

	it('does not mutate metadata when skipping queue item', async () => {
		await initLookup();

		const beforeFirst = context.metadataByFile.get('/books/alpha.m4b');
		const beforeSecond = context.metadataByFile.get('/books/beta.m4b');

		click('metadata-lookup-skip-btn');
		await flushAsync();

		expect(context.metadataByFile.get('/books/alpha.m4b')).toEqual(beforeFirst);
		expect(context.metadataByFile.get('/books/beta.m4b')).toEqual(beforeSecond);
	});
});
