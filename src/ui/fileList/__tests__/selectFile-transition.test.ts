import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { FileListInfo } from '../../../types/audio';
import { setCurrentFileList } from '../state.svelte';

const context = vi.hoisted(() => ({
	coordinate: vi.fn(async (_intent, mutate: (intent: unknown) => { changed: boolean }) => {
		mutate(_intent);
		return true;
	}),
	applySelectionIntent: vi.fn(() => ({ changed: true })),
}));

vi.mock('../metadataPanel', () => ({
	coordinateMetadataSurfaceSelectionTransition: context.coordinate,
	getSelectedFiles: vi.fn(() => []),
	showSingleSelection: vi.fn(),
	showMultiSelection: vi.fn(),
	clearSelectionPanels: vi.fn(),
	autoUpdateCoverArtFromFirstValidFile: vi.fn(),
	ensureMetadataForFiles: vi.fn(),
	coordinateMetadataSurfacePresentationRefresh: vi.fn(),
}));
vi.mock('../selection', () => ({
	applySelectionIntent: context.applySelectionIntent,
	reindexSelectionAfterMove: vi.fn(),
	reindexSelectionAfterRemoval: vi.fn(),
	swapSelectionIndices: vi.fn(),
}));
vi.mock('../../metadataSession', () => ({
	clearMetadataSession: vi.fn(),
	removeMetadataForFile: vi.fn(),
}));
vi.mock('../../outputPanel', () => ({ updateEstimatedSize: vi.fn() }));
vi.mock('../../remoteSource', () => ({ purgeRemoteSourceSessionsForInputIds: vi.fn() }));
vi.mock('../metadataStaging', () => ({ preserveMetadataDraftsBeforeSelectionChange: vi.fn() }));

describe('FileList selection delegation', () => {
	beforeEach(() => {
		setCurrentFileList({
			files: [
				{ path: '/books/alpha.m4b', size: 1, duration: 1, isValid: true },
				{ path: '/books/beta.m4b', size: 1, duration: 1, isValid: true },
			],
			selectedDecoders: [null, null],
			totalDuration: 2,
			totalSize: 2,
			validCount: 2,
			invalidCount: 0,
		} satisfies FileListInfo);
		context.coordinate.mockClear();
		context.applySelectionIntent.mockClear();
	});

	it('routes title, checkbox, and select-all intents through the metadata-surface coordinator', async () => {
		const { applySelectionIntent, selectAll } = await import('../actions');
		await applySelectionIntent({ type: 'selectOnly', index: 1 }, { openMetadataSurface: true });
		await applySelectionIntent({ type: 'toggle', index: 1 });
		await selectAll();

		expect(context.coordinate).toHaveBeenNthCalledWith(
			1,
			{ type: 'selectOnly', index: 1 },
			expect.any(Function),
			expect.objectContaining({ openAfterPopulate: true }),
		);
		expect(context.coordinate).toHaveBeenNthCalledWith(
			2,
			{ type: 'toggle', index: 1 },
			expect.any(Function),
			expect.objectContaining({ openAfterPopulate: undefined }),
		);
		expect(context.coordinate).toHaveBeenNthCalledWith(
			3,
			{ type: 'selectAll' },
			expect.any(Function),
			expect.any(Object),
		);
	});
});
