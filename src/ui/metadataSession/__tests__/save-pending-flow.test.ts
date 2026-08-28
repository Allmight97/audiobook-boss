import { beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';
import { get, type Writable } from 'svelte/store';

import type { MetadataSaveBatchResult } from '../../../types/metadata';
import { runtimeSettingsCapabilitiesFixture } from '../../../test/fixtures/runtimeSettingsCapabilities';

const context = vi.hoisted(() => ({
	saveMetadataBatchMock: vi.fn(),
	getRuntimeSettingsCapabilitiesMock: vi.fn(),
	persistPendingDraftsMock: vi.fn(async () => false),
	getPendingIntentEntriesMock: vi.fn(() => [] as Array<[string, Record<string, unknown>]>),
	clearPendingMock: vi.fn(),
	resetDirtyStateMock: vi.fn(),
	getCurrentFileListMock: vi.fn(() => ({
		files: [
			{ path: '/books/a.m4b', isValid: true },
			{ path: '/books/b.m4b', isValid: true },
		],
	})),
	metadataSaveInProgress: false,
	statusPanelProcessing: false,
}));

vi.mock('../../../lib/tauri/client', () => ({
	tauriClient: {
		saveMetadataBatch: context.saveMetadataBatchMock,
		getRuntimeSettingsCapabilities: context.getRuntimeSettingsCapabilitiesMock,
		listen: vi.fn(async () => () => {}),
		openFiles: vi.fn(),
		openFile: vi.fn(),
		openDirectory: vi.fn(),
		analyzeAudioFiles: vi.fn(async () => ({
			files: [],
			totalDuration: 0,
			totalSize: 0,
			validCount: 0,
			invalidCount: 0,
		})),
		readAudioMetadata: vi.fn(async () => ({})),
		listWorkOperations: vi.fn(async () => ({ operations: [] })),
		getWorkOperation: vi.fn(),
		cancelWorkOperation: vi.fn(),
		submitProcessingOperation: vi.fn(),
	},
}));

vi.mock('../../coverArt', () => ({
	getCurrentCoverArt: vi.fn(() => null),
	onLoadCoverArtFromFilePicker: vi.fn(),
	onLoadCoverArtFromInput: vi.fn(),
	onClearCoverArt: vi.fn(),
}));
vi.mock('../../jobControls', () => ({
	initJobControls: vi.fn(),
	handleMergeModeChange: vi.fn(),
	handleMaxConcurrentSelectionChange: vi.fn(),
	getMaxConcurrentStatus: vi.fn(() => ({ effective: 2, selection: 'auto' })),
}));
vi.mock('../../statusPanel/index', () => ({
	initStatusPanel: vi.fn(),
	isStatusPanelProcessing: () => context.statusPanelProcessing,
	pushStatusPanelTransientStatus: (message: string) => {
		const status = document.getElementById('status-text');
		if (status instanceof HTMLElement) {
			status.textContent = message;
		}
	},
}));

vi.mock('../../metadataForm', async (importOriginal) => {
	const actual = await importOriginal<typeof import('../../metadataForm')>();
	return {
		...actual,
		readMetadataFormViewSnapshot: vi.fn(() => ({ mode: 'single', selectionCount: 1 })),
		readMetadataFormPreviewValues: vi.fn(() => ({
			title: '',
			author: '',
			narrator: '',
			series: '',
			seriesPart: '',
			subseries: '',
			subseriesPart: '',
			year: '',
			genre: '',
		})),
		initMetadataFormEvents: vi.fn(),
		readMetadataForm: vi.fn(() => ({})),
		resetDirtyState: context.resetDirtyStateMock,
		onMetadataFormFieldInput: vi.fn(),
		onMetadataFormActionSelectChange: vi.fn(),
	};
});

vi.mock('../../fileList/state', () => ({
	getCurrentFileList: context.getCurrentFileListMock,
	getSelectedFileIndices: vi.fn(() => new Set<number>()),
	getSortAscending: vi.fn(() => true),
	getSortDirection: vi.fn(() => 'ascending'),
	getImportOrdinal: vi.fn(() => 0),
	isOrderLocked: vi.fn(() => false),
	onOrderLockChange: vi.fn(() => () => undefined),
}));

vi.mock('../../fileList/actions', () => ({
	appendFileList: vi.fn(),
}));

vi.mock('../../fileList/metadataStaging', () => ({
	persistPendingMetadataDraftsForCurrentSelection: context.persistPendingDraftsMock,
}));

vi.mock('../state', async (importOriginal) => ({
	...(await importOriginal<typeof import('../state')>()),
	getPendingMetadataIntentEntries: () => context.getPendingIntentEntriesMock(),
	clearPendingMetadataForFile: context.clearPendingMock,
}));

function getStatusText(): HTMLElement {
	const status = document.getElementById('status-text');
	if (!(status instanceof HTMLElement)) {
		throw new Error('status-text not found');
	}
	return status;
}

describe('metadata save pending flow', () => {
	let saveMetadataFromUI: typeof import('../saveWorkflow').saveMetadataFromUI;
	let metadataSaveInProgressStore: Writable<boolean>;

	beforeAll(async () => {
		document.body.innerHTML = `
      <div id="app"></div>
      <button id="metadata-save-btn">Save All Changes</button>
      <div id="status-text">Idle</div>
    `;
		context.getRuntimeSettingsCapabilitiesMock.mockResolvedValue(
			runtimeSettingsCapabilitiesFixture(),
		);

		await import('../../../main');
		({ saveMetadataFromUI } = await import('../saveWorkflow'));
		({ metadataSaveInProgressStore } = await import('../saveState'));
	});

	beforeEach(() => {
		context.saveMetadataBatchMock.mockReset();
		context.getRuntimeSettingsCapabilitiesMock.mockReset();
		context.getRuntimeSettingsCapabilitiesMock.mockResolvedValue(
			runtimeSettingsCapabilitiesFixture(),
		);
		context.persistPendingDraftsMock.mockReset();
		context.clearPendingMock.mockReset();
		context.resetDirtyStateMock.mockReset();
		context.getPendingIntentEntriesMock.mockReset();
		context.metadataSaveInProgress = false;
		context.statusPanelProcessing = false;
		context.getCurrentFileListMock.mockReturnValue({
			files: [
				{ path: '/books/a.m4b', isValid: true },
				{ path: '/books/b.m4b', isValid: true },
			],
		});
		metadataSaveInProgressStore.set(false);
		getStatusText().textContent = 'Idle';
	});

	it('blocks repeat save attempts synchronously while a save is in progress', async () => {
		let resolveSave: (result: MetadataSaveBatchResult) => void = () => {};
		context.persistPendingDraftsMock.mockResolvedValue(true);
		context.getPendingIntentEntriesMock.mockReturnValue([
			['/books/a.m4b', { title: { op: 'set', value: 'A' } }],
		]);
		context.saveMetadataBatchMock.mockImplementation(
			() =>
				new Promise((resolve) => {
					resolveSave = resolve;
				}),
		);

		const firstSave = saveMetadataFromUI();

		expect(get(metadataSaveInProgressStore)).toBe(true);

		await saveMetadataFromUI();

		await vi.waitFor(() => {
			expect(context.saveMetadataBatchMock).toHaveBeenCalledTimes(1);
		});
		expect(getStatusText().textContent).toBe('Save already in progress...');

		resolveSave({
			summary: { total: 1, succeeded: 1, skipped: 0, cancelled: 0, failed: 0 },
			results: [
				{
					inputIndex: 0,
					filePath: '/books/a.m4b',
					status: 'success',
					message: 'saved',
				},
			],
		});
		await firstSave;

		expect(context.saveMetadataBatchMock).toHaveBeenCalledTimes(1);
		expect(context.clearPendingMock).toHaveBeenCalledWith('/books/a.m4b');
		expect(context.resetDirtyStateMock).toHaveBeenCalledTimes(1);
		expect(get(metadataSaveInProgressStore)).toBe(false);
	});
});
