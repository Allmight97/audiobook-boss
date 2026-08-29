import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { FileListInfo } from '../../../types/audio';
import { setCurrentFileList, setSelectedIndex, setSelectedFileIndices } from '../state.svelte';

const context = vi.hoisted(() => ({
	readMetadataFormMock: vi.fn<() => Record<string, unknown>>(() => ({ title: 'Persisted Title' })),
	stageMetadataIntentPatchMock: vi.fn(() => 'staged' as const),
	resetDirtyStateMock: vi.fn(),
	handleSelectionMock: vi.fn(() => ({ changed: true })),
	selectAllFilesMock: vi.fn(() => true),
	pushStatusPanelTransientStatusMock: vi.fn(),
	validationErrorMock: vi.fn<() => string | null>(() => null),
	validateMetadataDraftMock: vi.fn(),
	clearSelectionMock: vi.fn(() => true),
	metadataFormRevision: 0,
	coverArtRevision: 0,
}));

vi.mock('../../metadataForm', () => ({
	hasDirtyMetadataFields: vi.fn(() => true),
	readMetadataForm: context.readMetadataFormMock,
	readMetadataFormRevision: vi.fn(() => context.metadataFormRevision),
	resetDirtyState: context.resetDirtyStateMock,
}));

vi.mock('../../coverArt', () => ({
	readCoverArtSessionRevision: vi.fn(() => context.coverArtRevision),
}));

vi.mock('../../metadataSession', () => ({
	clearMetadataSession: vi.fn(),
	getMetadataForFile: vi.fn(() => ({})),
	getMetadataIntentPatchForFile: vi.fn(() => undefined),
	isUsableMetadataCache: vi.fn(() => true),
	cacheMetadataForFile: vi.fn(),
	removeMetadataForFile: vi.fn(),
	stageMetadataIntentPatch: context.stageMetadataIntentPatchMock,
	validateMetadataDraft: context.validateMetadataDraftMock,
	metadataSaveInProgress: { subscribe: vi.fn() },
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
		setSelectedFileIndices([0]);

		context.readMetadataFormMock.mockClear();
		context.stageMetadataIntentPatchMock.mockClear();
		context.resetDirtyStateMock.mockClear();
		context.handleSelectionMock.mockClear();
		context.selectAllFilesMock.mockClear();
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
		context.metadataFormRevision = 0;
		context.coverArtRevision = 0;
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
	});

	it('keeps the current selection when clear-selection staging validation fails', async () => {
		context.validationErrorMock.mockReturnValue('Series part must be a number');
		setSelectedFileIndices([0, 1]);
		const { clearSelectionAction } = await import('../actions');

		await clearSelectionAction();

		expect(context.clearSelectionMock).not.toHaveBeenCalled();
		expect(context.pushStatusPanelTransientStatusMock).toHaveBeenCalledWith(
			'Fix metadata validation errors before clearing the selection.',
			expect.objectContaining({ ttlMs: 2500 }),
		);
	});

	it('lets the latest selection intent win when validation resolves out of order', async () => {
		type Deferred<T> = { promise: Promise<T>; resolve: (value: T) => void };
		const deferred = <T>(): Deferred<T> => {
			let resolve!: (value: T) => void;
			const promise = new Promise<T>((resolvePromise) => {
				resolve = resolvePromise;
			});
			return { promise, resolve };
		};
		const firstValidation = deferred<{
			intentPatch: Record<string, unknown>;
			ok: boolean;
			errors: { first: null; byField: Record<string, unknown> };
			result: { isValid: boolean; metadataPatch: Record<string, unknown>; fieldErrors: never[] };
		}>();
		const secondValidation =
			deferred<typeof firstValidation extends Deferred<infer T> ? T : never>();
		context.validateMetadataDraftMock
			.mockImplementationOnce(() => firstValidation.promise)
			.mockImplementationOnce(() => secondValidation.promise);

		const { selectFile } = await import('../actions');
		const first = selectFile(1, { multi: false, range: false });
		const second = selectFile(0, { multi: false, range: false });

		secondValidation.resolve({
			intentPatch: { title: { op: 'set', value: 'Latest' } },
			ok: true,
			errors: { first: null, byField: {} },
			result: { isValid: true, metadataPatch: {}, fieldErrors: [] },
		});
		await second;
		firstValidation.resolve({
			intentPatch: { title: { op: 'set', value: 'Stale' } },
			ok: true,
			errors: { first: null, byField: {} },
			result: { isValid: true, metadataPatch: {}, fieldErrors: [] },
		});
		await first;

		expect(context.handleSelectionMock).toHaveBeenCalledTimes(1);
		expect(context.handleSelectionMock).toHaveBeenCalledWith(0, {
			multi: false,
			range: false,
		});
		expect(context.stageMetadataIntentPatchMock).toHaveBeenCalledTimes(1);
		expect(context.resetDirtyStateMock).toHaveBeenCalledTimes(1);
	});

	it('does not publish validation status from a stale selection intent', async () => {
		type ValidationResult = {
			intentPatch: Record<string, unknown>;
			ok: boolean;
			errors: { first: string | null; byField: Record<string, unknown> };
			result: { isValid: boolean; metadataPatch: Record<string, unknown>; fieldErrors: never[] };
		};
		type Deferred<T> = { promise: Promise<T>; resolve: (value: T) => void };
		const deferred = <T>(): Deferred<T> => {
			let resolve!: (value: T) => void;
			const promise = new Promise<T>((resolvePromise) => {
				resolve = resolvePromise;
			});
			return { promise, resolve };
		};
		const staleValidation = deferred<ValidationResult>();
		const latestValidation = deferred<ValidationResult>();
		context.validateMetadataDraftMock
			.mockImplementationOnce(() => staleValidation.promise)
			.mockImplementationOnce(() => latestValidation.promise);

		const { selectFile } = await import('../actions');
		const stale = selectFile(1, { multi: false, range: false });
		const latest = selectFile(0, { multi: false, range: false });

		latestValidation.resolve({
			intentPatch: { title: { op: 'set', value: 'Latest' } },
			ok: true,
			errors: { first: null, byField: {} },
			result: { isValid: true, metadataPatch: {}, fieldErrors: [] },
		});
		await latest;
		context.pushStatusPanelTransientStatusMock.mockClear();
		staleValidation.resolve({
			intentPatch: {},
			ok: false,
			errors: { first: 'Stale validation failure', byField: {} },
			result: { isValid: false, metadataPatch: {}, fieldErrors: [] },
		});
		await stale;

		expect(context.pushStatusPanelTransientStatusMock).not.toHaveBeenCalled();
	});

	it('does not commit or change selection when the visible metadata form changes during validation', async () => {
		let resolveValidation!: (value: {
			intentPatch: Record<string, unknown>;
			ok: boolean;
			errors: { first: null; byField: Record<string, unknown> };
			result: { isValid: boolean; metadataPatch: Record<string, unknown>; fieldErrors: never[] };
		}) => void;
		context.validateMetadataDraftMock.mockImplementationOnce(
			() =>
				new Promise((resolve) => {
					resolveValidation = resolve;
				}),
		);

		const { selectFile } = await import('../actions');
		const transition = selectFile(1, { multi: false, range: false });
		context.metadataFormRevision += 1;
		resolveValidation({
			intentPatch: { title: { op: 'set', value: 'Stale' } },
			ok: true,
			errors: { first: null, byField: {} },
			result: { isValid: true, metadataPatch: {}, fieldErrors: [] },
		});
		await transition;

		expect(context.stageMetadataIntentPatchMock).not.toHaveBeenCalled();
		expect(context.resetDirtyStateMock).not.toHaveBeenCalled();
		expect(context.handleSelectionMock).not.toHaveBeenCalled();
	});

	it('does not apply a stale selection index after the FileList changes during validation', async () => {
		let resolveValidation!: (value: {
			intentPatch: Record<string, unknown>;
			ok: boolean;
			errors: { first: null; byField: Record<string, unknown> };
			result: { isValid: boolean; metadataPatch: Record<string, unknown>; fieldErrors: never[] };
		}) => void;
		context.validateMetadataDraftMock.mockImplementationOnce(
			() =>
				new Promise((resolve) => {
					resolveValidation = resolve;
				}),
		);

		const { selectFile } = await import('../actions');
		const transition = selectFile(1, { multi: false, range: false });
		setCurrentFileList({
			files: [],
			selectedDecoders: [],
			totalDuration: 0,
			totalSize: 0,
			validCount: 0,
			invalidCount: 0,
		});
		resolveValidation({
			intentPatch: { title: { op: 'set', value: 'Stale' } },
			ok: true,
			errors: { first: null, byField: {} },
			result: { isValid: true, metadataPatch: {}, fieldErrors: [] },
		});
		await transition;

		expect(context.stageMetadataIntentPatchMock).not.toHaveBeenCalled();
		expect(context.handleSelectionMock).not.toHaveBeenCalled();
	});

	it('stages dirty multi-selection metadata before selecting all files', async () => {
		context.readMetadataFormMock.mockReturnValue({ series: 'Draft Series' });
		setSelectedFileIndices([0, 1]);
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
	});
});
