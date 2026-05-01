import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { AudioFile, FileListInfo } from '../../types/audio';
import { moveFileDown, moveFileUp, reorderFiles } from '../fileList/actions';
import {
	getSelectedFileIndices,
	setCurrentFileList,
	setSelectedFileIndices,
	setSelectedIndex,
} from '../fileList/state.svelte';

vi.mock('../metadataForm', () => ({
	hasDirtyMetadataFields: vi.fn(() => false),
	readMetadataForm: vi.fn(() => ({})),
	resetDirtyState: vi.fn(),
}));

vi.mock('../metadataState', () => ({
	clearMetadataState: vi.fn(),
	getMetadataForFile: vi.fn(),
	metadataEqualsNullish: vi.fn(),
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

vi.mock('../fileList/dom', () => ({
	updateFileListDOM: vi.fn(),
	updateTotalStats: vi.fn(),
	updateSelection: vi.fn(),
	updateSortButtonText: vi.fn(),
	updateButtonVisibility: vi.fn(),
	showEmptyState: vi.fn(),
	setOrderLockNotice: vi.fn(),
}));

vi.mock('../statusPanel', () => ({
	pushStatusPanelTransientStatus: vi.fn(),
}));

vi.mock('../fileList/metadataPanel', async () => {
	const state = await vi.importActual<typeof import('../fileList/state.svelte')>(
		'../fileList/state.svelte',
	);
	return {
		autoUpdateCoverArtFromFirstValidFile: vi.fn(async () => undefined),
		clearSelectionPanels: vi.fn(),
		ensureMetadataForFiles: vi.fn(async () => undefined),
		getSelectedFiles: () => {
			const fileList = state.getCurrentFileList();
			if (!fileList) return [];
			return Array.from(state.getSelectedFileIndices())
				.map((index) => fileList.files[index])
				.filter((file): file is AudioFile => Boolean(file));
		},
		showMultiSelection: vi.fn(async () => undefined),
		showSingleSelection: vi.fn(async () => undefined),
	};
});

const makeFileList = (count: number): FileListInfo => ({
	files: Array.from({ length: count }, (_, index) => ({
		path: `/tmp/book-${index}.m4b`,
		isValid: true,
		size: 1,
		bitrate: 64,
		sampleRate: 44100,
		channels: 2,
		format: 'M4B',
		duration: 1,
	})),
	selectedDecoders: Array.from({ length: count }, () => null),
	totalDuration: count,
	totalSize: count,
	validCount: count,
	invalidCount: 0,
});

describe('file list reorder mirror', () => {
	beforeEach(() => {
		setCurrentFileList(makeFileList(3));
	});

	it('publishes the mirror after moving a selected file up', () => {
		setSelectedFileIndices([2]);
		setSelectedIndex(2);

		moveFileUp(2);

		expect(Array.from(getSelectedFileIndices()).sort((a, b) => a - b)).toEqual([1]);
	});

	it('publishes the mirror after moving a selected file down', () => {
		setSelectedFileIndices([0]);
		setSelectedIndex(0);

		moveFileDown(0);

		expect(Array.from(getSelectedFileIndices()).sort((a, b) => a - b)).toEqual([1]);
	});

	it('publishes the mirror when reordering a selected file', () => {
		setSelectedFileIndices([0]);
		setSelectedIndex(0);

		reorderFiles(0, 2);

		expect(Array.from(getSelectedFileIndices()).sort((a, b) => a - b)).toEqual([2]);
	});
});
