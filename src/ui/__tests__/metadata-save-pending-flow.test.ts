import { beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';
import { writable } from 'svelte/store';

const context = vi.hoisted(() => ({
	saveMetadataIntentToFileMock: vi.fn(),
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
	saveMetadataHandler: null as null | (() => void),
}));

vi.mock('../../lib/tauri/client', () => ({
	tauriClient: {
		saveMetadataIntentToFile: context.saveMetadataIntentToFileMock,
		listen: vi.fn(async () => () => {}),
		open: vi.fn(),
		analyzeAudioFiles: vi.fn(async () => ({
			files: [],
			totalDuration: 0,
			totalSize: 0,
			validCount: 0,
			invalidCount: 0,
		})),
		readAudioMetadata: vi.fn(async () => ({})),
	},
}));

vi.mock('../fileImport', () => ({ initFileImport: vi.fn() }));
vi.mock('../outputPanel', () => ({ initOutputPanel: vi.fn() }));
vi.mock('../encoderPanel', () => ({ initEncoderPanel: vi.fn() }));
vi.mock('../coverArt', () => ({
	initCoverArt: vi.fn(),
	onLoadCoverArtFromFilePicker: vi.fn(),
	onLoadCoverArtFromInput: vi.fn(),
	onClearCoverArt: vi.fn(),
}));
vi.mock('../tagPreview', () => ({ initTagPreview: vi.fn() }));
vi.mock('../jobControls', () => ({
	initJobControls: vi.fn(),
	handleMergeModeChange: vi.fn(),
	handleMaxConcurrentSelectionChange: vi.fn(),
}));
vi.mock('../metadataLookup', () => ({ initMetadataLookup: vi.fn() }));
vi.mock('../statusPanel/index', () => ({
	initStatusPanel: vi.fn(),
	getStatusPanel: () => ({ isCurrentlyProcessing: false }),
	pushStatusPanelTransientStatus: (message: string) => {
		const status = document.getElementById('status-text');
		if (status instanceof HTMLElement) {
			status.textContent = message;
		}
	},
}));

vi.mock('../metadataForm', () => ({
	initMetadataFormEvents: vi.fn(() => {
		const saveButton = document.getElementById('metadata-save-btn');
		if (saveButton instanceof HTMLButtonElement) {
			saveButton.onclick = () => {
				context.saveMetadataHandler?.();
			};
		}
	}),
	resetDirtyState: context.resetDirtyStateMock,
	onMetadataFormFieldInput: vi.fn(),
	onMetadataFormActionSelectChange: vi.fn(),
	triggerMetadataFormSave: vi.fn(),
	setMetadataFormSaveHandler: (handler: () => void) => {
		context.saveMetadataHandler = handler;
	},
}));

vi.mock('../fileList', () => ({
	getCurrentFileList: context.getCurrentFileListMock,
}));

vi.mock('../fileList/actions', () => ({
	persistPendingMetadataDraftsForCurrentSelection: context.persistPendingDraftsMock,
}));

vi.mock('../metadataState', () => ({
	getPendingMetadataIntentEntries: () => context.getPendingIntentEntriesMock(),
	clearPendingMetadataForFile: context.clearPendingMock,
}));

vi.mock('../metadataSaveState', () => ({
	metadataSaveInProgressStore: writable(false),
	isMetadataSaveInProgress: () => context.metadataSaveInProgress,
	setMetadataSaveInProgress: (value: boolean) => {
		context.metadataSaveInProgress = value;
	},
}));

function getSaveButton(): HTMLButtonElement {
	const button = document.getElementById('metadata-save-btn');
	if (!(button instanceof HTMLButtonElement)) {
		throw new Error('metadata-save-btn not found');
	}
	return button;
}

function getStatusText(): HTMLElement {
	const status = document.getElementById('status-text');
	if (!(status instanceof HTMLElement)) {
		throw new Error('status-text not found');
	}
	return status;
}

describe('metadata save pending flow', () => {
	beforeAll(async () => {
		document.body.innerHTML = `
      <div id="app"></div>
      <button id="metadata-save-btn">Save All Changes</button>
      <div id="status-text">Idle</div>
    `;

		await import('../../main');
	});

	beforeEach(() => {
		context.saveMetadataIntentToFileMock.mockReset();
		context.persistPendingDraftsMock.mockReset();
		context.clearPendingMock.mockReset();
		context.resetDirtyStateMock.mockReset();
		context.getPendingIntentEntriesMock.mockReset();
		context.metadataSaveInProgress = false;
		context.getCurrentFileListMock.mockReturnValue({
			files: [
				{ path: '/books/a.m4b', isValid: true },
				{ path: '/books/b.m4b', isValid: true },
			],
		});
		getStatusText().textContent = 'Idle';
	});

	it('saves all pending entries and clears pending markers on success', async () => {
		context.persistPendingDraftsMock.mockResolvedValue(true);
		context.getPendingIntentEntriesMock.mockReturnValue([
			['/books/a.m4b', { title: { op: 'set', value: 'A' } }],
			['/books/b.m4b', { title: { op: 'set', value: 'B' } }],
		]);
		context.saveMetadataIntentToFileMock.mockResolvedValue(undefined);

		getSaveButton().click();

		await vi.waitFor(() => {
			expect(context.saveMetadataIntentToFileMock).toHaveBeenCalledTimes(2);
		});
		expect(context.persistPendingDraftsMock).toHaveBeenCalledWith({ showStatus: false });
		expect(context.clearPendingMock).toHaveBeenCalledTimes(2);
		expect(context.clearPendingMock).toHaveBeenCalledWith('/books/a.m4b');
		expect(context.clearPendingMock).toHaveBeenCalledWith('/books/b.m4b');
		expect(context.resetDirtyStateMock).toHaveBeenCalledTimes(1);
		expect(getStatusText().textContent).toContain('Metadata saved (2 files)!');
	});

	it('retains failed files in pending state and surfaces partial failure summary', async () => {
		context.persistPendingDraftsMock.mockResolvedValue(true);
		context.getPendingIntentEntriesMock.mockReturnValue([
			['/books/a.m4b', { title: { op: 'set', value: 'A' } }],
			['/books/b.m4b', { title: { op: 'set', value: 'B' } }],
		]);
		context.saveMetadataIntentToFileMock
			.mockResolvedValueOnce(undefined)
			.mockRejectedValueOnce(new Error('write failed'));

		getSaveButton().click();

		await vi.waitFor(() => {
			expect(context.saveMetadataIntentToFileMock).toHaveBeenCalledTimes(2);
		});
		expect(context.clearPendingMock).toHaveBeenCalledTimes(1);
		expect(context.clearPendingMock).toHaveBeenCalledWith('/books/a.m4b');
		expect(getStatusText().textContent).toBe('Saved 1/2. Failed: b.m4b');
	});

	it('shows explicit status when there are no pending metadata changes', async () => {
		context.persistPendingDraftsMock.mockResolvedValue(false);
		context.getPendingIntentEntriesMock.mockReturnValue([]);

		getSaveButton().click();

		await vi.waitFor(() => {
			expect(context.persistPendingDraftsMock).toHaveBeenCalledWith({
				showStatus: false,
			});
		});
		expect(context.saveMetadataIntentToFileMock).not.toHaveBeenCalled();
		await vi.waitFor(() => {
			expect(getStatusText().textContent).toBe('No pending metadata changes');
		});
	});
});
