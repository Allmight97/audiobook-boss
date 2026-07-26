import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { FileListInfo } from '../../../types/audio';
import { setCurrentFileList, setSelectedIndex } from '../state.svelte';

const context = vi.hoisted(() => ({
	readMetadataFormMock: vi.fn<() => Record<string, unknown>>(() => ({ title: 'Persisted Title' })),
	stageMetadataIntentPatchMock: vi.fn(() => 'staged' as const),
	getSelectedFilesMock: vi.fn(),
	handleSelectionMock: vi.fn(() => ({ changed: true })),
	selectAllFilesMock: vi.fn(() => true),
	showMultiSelectionMock: vi.fn(),
	showSingleSelectionMock: vi.fn(),
	pushStatusPanelTransientStatusMock: vi.fn(),
	validationErrorMock: vi.fn<() => string | null>(() => null),
	validateMetadataDraftMock: vi.fn(),
	clearSelectionMock: vi.fn(() => true),
}));

vi.mock('../../metadataForm', () => ({
	hasDirtyMetadataFields: vi.fn(() => true),
	readMetadataForm: context.readMetadataFormMock,
	resetDirtyState: vi.fn(),
}));

vi.mock('../../metadataSession', () => ({
	clearMetadataSession: vi.fn(),
	getMetadataForFile: vi.fn(() => ({})),
	cacheMetadataForFile: vi.fn(),
	removeMetadataForFile: vi.fn(),
	stageMetadataIntentPatch: context.stageMetadataIntentPatchMock,
	validateMetadataDraft: context.validateMetadataDraftMock,
	metadataSaveInProgress: { subscribe: vi.fn() },
}));

vi.mock('../../outputPanel', () => ({
	updateEstimatedSize: vi.fn(),
	updateOutputPath: vi.fn(),
}));

vi.mock('../events', () => ({
	initFileListEvents: vi.fn(),
	setupDragStartHandlers: vi.fn(),
}));

vi.mock('../selection', () => ({
	clearSelection: context.clearSelectionMock,
	handleSelection: context.handleSelectionMock,
	reindexSelectionAfterMove: vi.fn(),
	reindexSelectionAfterRemoval: vi.fn(),
	selectAllFiles: context.selectAllFilesMock,
	swapSelectionIndices: vi.fn(),
}));

vi.mock('../metadataPanel', () => ({
	autoUpdateCoverArtFromFirstValidFile: vi.fn(async () => undefined),
	clearSelectionPanels: vi.fn(),
	ensureMetadataForFiles: vi.fn(async () => undefined),
	getSelectedFiles: context.getSelectedFilesMock,
	refreshSelectionPresentation: vi.fn(),
	showMultiSelection: context.showMultiSelectionMock,
	showSingleSelection: context.showSingleSelectionMock,
}));

vi.mock('../../statusPanel', () => ({
	pushStatusPanelTransientStatus: context.pushStatusPanelTransientStatusMock,
}));

describe('selectFile transition options', () => {
	beforeEach(() => {
		document.body.innerHTML = `
      <form id="metadata-form">
        <input id="meta-title" data-dirty="true" />
      </form>
      <div id="status-text"></div>
    `;

		const fileList: FileListInfo = {
			files: [
				{
					path: '/books/alpha.m4b',
					size: 1,
					duration: 1,
					isValid: true,
					bitrate: 64,
					sampleRate: 44100,
					channels: 2,
				},
				{
					path: '/books/beta.m4b',
					size: 1,
					duration: 1,
					isValid: true,
					bitrate: 64,
					sampleRate: 44100,
					channels: 2,
				},
			],
			selectedDecoders: [null, null],
			totalDuration: 2,
			totalSize: 2,
			validCount: 2,
			invalidCount: 0,
		};

		setCurrentFileList(fileList);
		setSelectedIndex(0);

		context.readMetadataFormMock.mockClear();
		context.stageMetadataIntentPatchMock.mockClear();
		context.handleSelectionMock.mockClear();
		context.selectAllFilesMock.mockClear();
		context.showMultiSelectionMock.mockClear();
		context.showSingleSelectionMock.mockClear();
		context.pushStatusPanelTransientStatusMock.mockClear();
		context.validationErrorMock.mockReset();
		context.validateMetadataDraftMock.mockReset();
		context.validateMetadataDraftMock.mockImplementation(
			async (metadata: Record<string, unknown>) => {
				const first = context.validationErrorMock();
				return {
					intentPatch: Object.fromEntries(
						Object.entries(metadata).map(([key, value]) => [key, { op: 'set', value }]),
					),
					ok: first == null,
					errors: { first, byField: {} },
					result: { isValid: first == null, metadataPatch: {}, fieldErrors: [] },
				};
			},
		);
		context.validationErrorMock.mockReturnValue(null);
		context.clearSelectionMock.mockClear();
		context.getSelectedFilesMock.mockReset();
		context.getSelectedFilesMock.mockReturnValue([
			{
				path: '/books/alpha.m4b',
				isValid: true,
			},
		]);
	});

	it('skips previous-file autosave for queue-managed transitions', async () => {
		const { selectFile } = await import('../actions');
		await selectFile(1, { multi: false, range: false }, { skipPersistPrevious: true });

		expect(context.readMetadataFormMock).not.toHaveBeenCalled();
		expect(context.stageMetadataIntentPatchMock).not.toHaveBeenCalled();
	});

	it('preserves default autosave behavior when transition option is omitted', async () => {
		const { selectFile } = await import('../actions');
		await selectFile(1, { multi: false, range: false });

		expect(context.readMetadataFormMock).toHaveBeenCalledWith({ mode: 'single' });
		expect(context.stageMetadataIntentPatchMock).toHaveBeenCalledWith('/books/alpha.m4b', {
			title: { op: 'set', value: 'Persisted Title' },
		});
	});

	it('keeps the current selection when staging validation fails', async () => {
		context.validationErrorMock.mockReturnValue('Series part must be a number');
		const { selectFile } = await import('../actions');

		await selectFile(1, { multi: false, range: false });

		expect(context.handleSelectionMock).not.toHaveBeenCalled();
		expect(context.pushStatusPanelTransientStatusMock).toHaveBeenCalledWith(
			'Series part must be a number',
			expect.objectContaining({ ttlMs: 2500 }),
		);
		expect(context.showSingleSelectionMock).not.toHaveBeenCalled();
	});

	it('keeps the current selection when clear-selection staging validation fails', async () => {
		context.validationErrorMock.mockReturnValue('Series part must be a number');
		context.getSelectedFilesMock.mockReturnValue([
			{ path: '/books/alpha.m4b', isValid: true },
			{ path: '/books/beta.m4b', isValid: true },
		]);
		const { clearSelectionAction } = await import('../actions');

		await clearSelectionAction();

		expect(context.clearSelectionMock).not.toHaveBeenCalled();
		expect(context.pushStatusPanelTransientStatusMock).toHaveBeenCalledWith(
			'Fix metadata validation errors before clearing the selection.',
			expect.objectContaining({ ttlMs: 2500 }),
		);
	});

	it('stages dirty multi-selection metadata before selecting all files', async () => {
		context.readMetadataFormMock.mockReturnValue({ series: 'Draft Series' });
		context.getSelectedFilesMock.mockReturnValue([
			{ path: '/books/alpha.m4b', isValid: true },
			{ path: '/books/beta.m4b', isValid: true },
		]);
		const { selectAll } = await import('../actions');

		await selectAll();

		expect(context.readMetadataFormMock).toHaveBeenCalledWith({
			mode: 'multi',
			onlyDirty: true,
		});
		expect(context.stageMetadataIntentPatchMock).toHaveBeenCalledWith('/books/alpha.m4b', {
			series: { op: 'set', value: 'Draft Series' },
		});
		expect(context.stageMetadataIntentPatchMock).toHaveBeenCalledWith('/books/beta.m4b', {
			series: { op: 'set', value: 'Draft Series' },
		});
		expect(context.selectAllFilesMock).toHaveBeenCalledTimes(1);
		expect(context.showMultiSelectionMock).toHaveBeenCalledTimes(1);
	});
});
