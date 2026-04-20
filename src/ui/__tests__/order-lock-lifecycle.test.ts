import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { FileListInfo } from '../../types/audio';
import {
	isOrderLocked,
	setOrderLocked,
	setCurrentFileList,
	getCurrentFileList,
} from '../fileList/state';
import { fileListViewState } from '../fileList/viewState.svelte';

// Mock modules that actions.ts imports but aren't relevant to lock behavior
vi.mock('../metadataForm', () => ({
	hasDirtyMetadataFields: vi.fn(() => false),
	readMetadataForm: vi.fn(() => ({})),
	resetDirtyState: vi.fn(),
}));

vi.mock('../metadataState', () => ({
	clearMetadataState: vi.fn(),
	getMetadataForFile: vi.fn(() => ({})),
	metadataEqualsNullish: vi.fn(() => false),
	removeMetadataForFile: vi.fn(),
	setMetadataForFile: vi.fn(),
}));

vi.mock('../outputPanel', () => ({
	updateEstimatedSize: vi.fn(),
	updateOutputPath: vi.fn(),
}));

vi.mock('../metadataValidation', () => ({
	getSeriesPartValidationError: vi.fn(() => null),
	getSubseriesPartValidationError: vi.fn(() => null),
}));

vi.mock('../fileList/events', () => ({
	initFileListEvents: vi.fn(),
	setupDragStartHandlers: vi.fn(),
}));

vi.mock('../fileList/selection', () => ({
	clearSelection: vi.fn(() => true),
	handleSelection: vi.fn(() => ({ changed: true })),
	reindexSelectionAfterMove: vi.fn(),
	reindexSelectionAfterRemoval: vi.fn(),
	selectAllFiles: vi.fn(() => true),
	swapSelectionIndices: vi.fn(),
}));

vi.mock('../fileList/metadataPanel', () => ({
	autoUpdateCoverArtFromFirstValidFile: vi.fn(async () => undefined),
	clearSelectionPanels: vi.fn(),
	ensureMetadataForFiles: vi.fn(async () => undefined),
	getSelectedFiles: vi.fn(() => []),
	showMultiSelection: vi.fn(async () => undefined),
	showSingleSelection: vi.fn(async () => undefined),
}));

vi.mock('../fileImport', () => ({
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
			const { setFileOrderLocked } = await import('../fileList/actions');
			setCurrentFileList(makeFileList(2));
			setFileOrderLocked(true);
			expect(isOrderLocked()).toBe(true);
		});

		it('unlocks when setFileOrderLocked(false) is called', async () => {
			const { setFileOrderLocked } = await import('../fileList/actions');
			setCurrentFileList(makeFileList(2));
			setFileOrderLocked(true);
			expect(isOrderLocked()).toBe(true);
			setFileOrderLocked(false);
			expect(isOrderLocked()).toBe(false);
		});

		it('is idempotent — double lock/unlock does not break state', async () => {
			const { setFileOrderLocked } = await import('../fileList/actions');
			setCurrentFileList(makeFileList(2));
			setFileOrderLocked(true);
			setFileOrderLocked(true);
			expect(isOrderLocked()).toBe(true);
			setFileOrderLocked(false);
			setFileOrderLocked(false);
			expect(isOrderLocked()).toBe(false);
		});
	});

	describe('guard behavior — locked prevents mutations', () => {
		it('clearAllFiles() no-ops when locked', async () => {
			const { clearAllFiles, setFileOrderLocked } = await import('../fileList/actions');
			const fileList = makeFileList(3);
			setCurrentFileList(fileList);
			setFileOrderLocked(true);

			clearAllFiles();

			const current = getCurrentFileList();
			expect(current?.files.length).toBe(3);
		});

		it('removeFile() no-ops when locked', async () => {
			const { removeFile, setFileOrderLocked } = await import('../fileList/actions');
			const fileList = makeFileList(3);
			setCurrentFileList(fileList);
			setFileOrderLocked(true);

			removeFile(0);

			const current = getCurrentFileList();
			expect(current?.files.length).toBe(3);
		});

		it('moveFileUp() no-ops when locked', async () => {
			const { moveFileUp, setFileOrderLocked } = await import('../fileList/actions');
			const fileList = makeFileList(3);
			setCurrentFileList(fileList);
			const originalOrder = fileList.files.map((f) => f.path);
			setFileOrderLocked(true);

			moveFileUp(1);

			const current = getCurrentFileList();
			expect(current?.files.map((f) => f.path)).toEqual(originalOrder);
		});

		it('moveFileDown() no-ops when locked', async () => {
			const { moveFileDown, setFileOrderLocked } = await import('../fileList/actions');
			const fileList = makeFileList(3);
			setCurrentFileList(fileList);
			const originalOrder = fileList.files.map((f) => f.path);
			setFileOrderLocked(true);

			moveFileDown(0);

			const current = getCurrentFileList();
			expect(current?.files.map((f) => f.path)).toEqual(originalOrder);
		});

		it('toggleFileSort() no-ops when locked', async () => {
			const { toggleFileSort, setFileOrderLocked } = await import('../fileList/actions');
			const fileList = makeFileList(3);
			setCurrentFileList(fileList);
			const originalOrder = fileList.files.map((f) => f.path);
			setFileOrderLocked(true);

			toggleFileSort();

			const current = getCurrentFileList();
			expect(current?.files.map((f) => f.path)).toEqual(originalOrder);
		});

		it('reorderFiles() no-ops when locked', async () => {
			const { reorderFiles, setFileOrderLocked } = await import('../fileList/actions');
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
		it('disables sort and clear controls when locked', async () => {
			const { initDOMCache, updateButtonVisibility } = await import('../fileList/dom');
			setCurrentFileList(makeFileList(3));
			initDOMCache();
			setOrderLocked(true);

			updateButtonVisibility();

			expect(fileListViewState.sortDisabled).toBe(true);
			expect(fileListViewState.clearDisabled).toBe(true);
		});

		it('enables sort and clear controls when unlocked', async () => {
			const { initDOMCache, updateButtonVisibility } = await import('../fileList/dom');
			setCurrentFileList(makeFileList(3));
			initDOMCache();
			setOrderLocked(false);

			updateButtonVisibility();

			expect(fileListViewState.sortDisabled).toBe(false);
			expect(fileListViewState.clearDisabled).toBe(false);
		});

		it('shows lock notice when locked', async () => {
			const { initDOMCache, setOrderLockNotice } = await import('../fileList/dom');
			initDOMCache();

			setOrderLockNotice(true);

			expect(fileListViewState.orderLockVisible).toBe(true);
		});

		it('hides lock notice when unlocked', async () => {
			const { initDOMCache, setOrderLockNotice } = await import('../fileList/dom');
			initDOMCache();
			setOrderLockNotice(true);

			setOrderLockNotice(false);

			expect(fileListViewState.orderLockVisible).toBe(false);
		});
	});

	describe('round-trip: lock → unlock restores full interactivity', () => {
		it('mutations work after lock/unlock cycle', async () => {
			const { setFileOrderLocked, clearAllFiles } = await import('../fileList/actions');
			setCurrentFileList(makeFileList(3));

			// Lock then unlock
			setFileOrderLocked(true);
			setFileOrderLocked(false);

			// clearAllFiles should now work
			clearAllFiles();

			const current = getCurrentFileList();
			expect(current?.files.length).toBe(0);
		});

		it('view controls re-enabled after lock/unlock cycle', async () => {
			const { initDOMCache, updateButtonVisibility } = await import('../fileList/dom');
			setCurrentFileList(makeFileList(3));
			initDOMCache();

			// Lock
			setOrderLocked(true);
			updateButtonVisibility();
			expect(fileListViewState.clearDisabled).toBe(true);

			// Unlock
			setOrderLocked(false);
			updateButtonVisibility();
			expect(fileListViewState.clearDisabled).toBe(false);
		});
	});
});
