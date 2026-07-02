import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { FileListInfo } from '../../../types/audio';
import {
	getCurrentFileList,
	isOrderLocked,
	onOrderLockChange,
	setCurrentFileList,
	setOrderLocked,
} from '../state.svelte';
import { readFileListControlsSnapshot, readFileListOrderLockVisible } from '../viewState.svelte';

// Mock modules that actions.ts imports but aren't relevant to lock behavior
vi.mock('../../metadataForm', () => ({
	hasDirtyMetadataFields: vi.fn(() => false),
	readMetadataForm: vi.fn(() => ({})),
	resetDirtyState: vi.fn(),
}));

vi.mock('../../metadataSession', () => ({
	clearMetadataSession: vi.fn(),
	getMetadataForFile: vi.fn(() => ({})),
	cacheMetadataForFile: vi.fn(),
	getMetadataIntentPatchForFile: vi.fn(() => undefined),
	isUsableMetadataCache: vi.fn(() => true),
	removeMetadataForFile: vi.fn(),
	stageMetadataIntentPatch: vi.fn(() => 'noop'),
	validateMetadataDraft: vi.fn(async () => ({
		intentPatch: {},
		ok: true,
		errors: { first: null, byField: {} },
		result: { isValid: true, metadataPatch: {}, fieldErrors: [] },
	})),
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
	clearSelection: vi.fn(() => true),
	handleSelection: vi.fn(() => ({ changed: true })),
	reindexSelectionAfterMove: vi.fn(),
	reindexSelectionAfterRemoval: vi.fn(),
	selectAllFiles: vi.fn(() => true),
	swapSelectionIndices: vi.fn(),
}));

vi.mock('../metadataPanel', () => ({
	autoUpdateCoverArtFromFirstValidFile: vi.fn(async () => undefined),
	clearSelectionPanels: vi.fn(),
	ensureMetadataForFiles: vi.fn(async () => undefined),
	getSelectedFiles: vi.fn(() => []),
	refreshSelectionPresentation: vi.fn(),
	showMultiSelection: vi.fn(async () => undefined),
	showSingleSelection: vi.fn(async () => undefined),
}));

vi.mock('../../fileImport', () => ({
	updateDropZoneState: vi.fn(),
}));

function makeFileList(count: number): FileListInfo {
	return {
		files: Array.from({ length: count }, (_, i) => ({
			path: `/tmp/file-${i}.m4b`,
			isValid: true,
			size: 1000,
			duration: 300,
			bitrate: 64,
			sampleRate: 44100,
			channels: 2,
			format: 'M4B',
		})),
		selectedDecoders: Array.from({ length: count }, () => null),
		totalDuration: count * 300,
		totalSize: count * 1000,
		validCount: count,
		invalidCount: 0,
	};
}

describe('order lock lifecycle', () => {
	beforeEach(() => {
		// Reset lock state between tests
		setOrderLocked(false);
		setCurrentFileList(null);
	});

	describe('state transitions', () => {
		it('starts unlocked', () => {
			expect(isOrderLocked()).toBe(false);
		});

		it('locks when setFileOrderLocked(true) is called', async () => {
			const { setFileOrderLocked } = await import('../actions');
			setCurrentFileList(makeFileList(2));
			setFileOrderLocked(true);
			expect(isOrderLocked()).toBe(true);
		});

		it('unlocks when setFileOrderLocked(false) is called', async () => {
			const { setFileOrderLocked } = await import('../actions');
			setCurrentFileList(makeFileList(2));
			setFileOrderLocked(true);
			expect(isOrderLocked()).toBe(true);
			setFileOrderLocked(false);
			expect(isOrderLocked()).toBe(false);
		});

		it('is idempotent — double lock/unlock does not break state', async () => {
			const { setFileOrderLocked } = await import('../actions');
			setCurrentFileList(makeFileList(2));
			setFileOrderLocked(true);
			setFileOrderLocked(true);
			expect(isOrderLocked()).toBe(true);
			setFileOrderLocked(false);
			setFileOrderLocked(false);
			expect(isOrderLocked()).toBe(false);
		});

		it('notifies order lock listeners only on lock transitions', () => {
			const listener = vi.fn();
			const unlisten = onOrderLockChange(listener);

			setOrderLocked(true);
			setOrderLocked(true);
			setOrderLocked(false);
			unlisten();
			setOrderLocked(true);

			expect(listener).toHaveBeenCalledTimes(2);
			expect(listener).toHaveBeenNthCalledWith(1, true);
			expect(listener).toHaveBeenNthCalledWith(2, false);
		});
	});

	describe('guard behavior — locked prevents mutations', () => {
		it('clearAllFiles() no-ops when locked', async () => {
			const { clearAllFiles, setFileOrderLocked } = await import('../actions');
			const fileList = makeFileList(3);
			setCurrentFileList(fileList);
			setFileOrderLocked(true);

			clearAllFiles();

			const current = getCurrentFileList();
			expect(current?.files.length).toBe(3);
		});

		it('removeFile() no-ops when locked', async () => {
			const { removeFile, setFileOrderLocked } = await import('../actions');
			const fileList = makeFileList(3);
			setCurrentFileList(fileList);
			setFileOrderLocked(true);

			removeFile(0);

			const current = getCurrentFileList();
			expect(current?.files.length).toBe(3);
		});

		it('moveFileUp() no-ops when locked', async () => {
			const { moveFileUp, setFileOrderLocked } = await import('../actions');
			const fileList = makeFileList(3);
			setCurrentFileList(fileList);
			const originalOrder = fileList.files.map((f) => f.path);
			setFileOrderLocked(true);

			moveFileUp(1);

			const current = getCurrentFileList();
			expect(current?.files.map((f) => f.path)).toEqual(originalOrder);
		});

		it('moveFileDown() no-ops when locked', async () => {
			const { moveFileDown, setFileOrderLocked } = await import('../actions');
			const fileList = makeFileList(3);
			setCurrentFileList(fileList);
			const originalOrder = fileList.files.map((f) => f.path);
			setFileOrderLocked(true);

			moveFileDown(0);

			const current = getCurrentFileList();
			expect(current?.files.map((f) => f.path)).toEqual(originalOrder);
		});

		it('toggleFileSort() no-ops when locked', async () => {
			const { toggleFileSort, setFileOrderLocked } = await import('../actions');
			const fileList = makeFileList(3);
			setCurrentFileList(fileList);
			const originalOrder = fileList.files.map((f) => f.path);
			setFileOrderLocked(true);

			toggleFileSort();

			const current = getCurrentFileList();
			expect(current?.files.map((f) => f.path)).toEqual(originalOrder);
		});

		it('reorderFiles() no-ops when locked', async () => {
			const { reorderFiles, setFileOrderLocked } = await import('../actions');
			const fileList = makeFileList(3);
			setCurrentFileList(fileList);
			const originalOrder = fileList.files.map((f) => f.path);
			setFileOrderLocked(true);

			reorderFiles(0, 2);

			const current = getCurrentFileList();
			expect(current?.files.map((f) => f.path)).toEqual(originalOrder);
		});
	});

	describe('view state reflects lock', () => {
		it('disables sort and clear controls when locked', () => {
			setCurrentFileList(makeFileList(3));
			setOrderLocked(true);

			const controls = readFileListControlsSnapshot();
			expect(controls.sortDisabled).toBe(true);
			expect(controls.clearDisabled).toBe(true);
		});

		it('enables sort and clear controls when unlocked', () => {
			setCurrentFileList(makeFileList(3));
			setOrderLocked(false);

			const controls = readFileListControlsSnapshot();
			expect(controls.sortDisabled).toBe(false);
			expect(controls.clearDisabled).toBe(false);
		});

		it('shows lock notice when locked', () => {
			setOrderLocked(true);
			expect(readFileListOrderLockVisible()).toBe(true);
		});

		it('hides lock notice when unlocked', () => {
			setOrderLocked(true);
			setOrderLocked(false);
			expect(readFileListOrderLockVisible()).toBe(false);
		});
	});

	describe('round-trip: lock → unlock restores full interactivity', () => {
		it('mutations work after lock/unlock cycle', async () => {
			const { setFileOrderLocked, clearAllFiles } = await import('../actions');
			setCurrentFileList(makeFileList(3));

			// Lock then unlock
			setFileOrderLocked(true);
			setFileOrderLocked(false);

			// clearAllFiles should now work
			clearAllFiles();

			const current = getCurrentFileList();
			expect(current?.files.length).toBe(0);
		});

		it('view controls re-enabled after lock/unlock cycle', () => {
			setCurrentFileList(makeFileList(3));

			setOrderLocked(true);
			expect(readFileListControlsSnapshot().clearDisabled).toBe(true);

			setOrderLocked(false);
			expect(readFileListControlsSnapshot().clearDisabled).toBe(false);
		});
	});
});
