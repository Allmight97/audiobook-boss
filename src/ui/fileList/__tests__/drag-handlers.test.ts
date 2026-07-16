import { beforeEach, describe, expect, it, vi } from 'vitest';
import { createFileListDragHandlers } from '../events';

const context = vi.hoisted(() => ({
	getCurrentFileListMock: vi.fn(),
	isOrderLockedMock: vi.fn(() => false),
	metadataSaveInProgress: { subscribe: vi.fn() },
}));

vi.mock('svelte/store', () => ({
	get: () => false,
}));

vi.mock('../state.svelte', () => ({
	getCurrentFileList: context.getCurrentFileListMock,
	isOrderLocked: context.isOrderLockedMock,
}));

vi.mock('../../metadataSession', () => ({
	metadataSaveInProgress: context.metadataSaveInProgress,
}));

vi.mock('../actions', () => ({
	reorderFiles: vi.fn(),
}));

function makeDragEvent(): DragEvent {
	return {
		preventDefault: vi.fn(),
		stopPropagation: vi.fn(),
		dataTransfer: {
			effectAllowed: 'move',
			dropEffect: 'move',
			setData: vi.fn(),
		},
		currentTarget: document.createElement('div'),
	} as unknown as DragEvent;
}

describe('createFileListDragHandlers', () => {
	beforeEach(() => {
		context.getCurrentFileListMock.mockReturnValue({
			files: [{ path: '/books/a.m4b' }, { path: '/books/b.m4b' }],
		});
	});

	it('does not highlight rows during external drag-over when no internal drag started', () => {
		const states: Array<{ draggedIndex: number | null; hoveredIndex: number | null }> = [];
		const handlers = createFileListDragHandlers((state) => {
			states.push(state);
		});

		handlers.onDragOver(1, makeDragEvent());

		expect(states).toHaveLength(0);
	});

	it('highlights a different row during internal reorder drag-over', () => {
		const states: Array<{ draggedIndex: number | null; hoveredIndex: number | null }> = [];
		const handlers = createFileListDragHandlers((state) => {
			states.push(state);
		});

		handlers.onDragStart(0, makeDragEvent());
		handlers.onDragOver(1, makeDragEvent());

		expect(states[states.length - 1]).toEqual({
			draggedIndex: 0,
			hoveredIndex: 1,
		});
	});

	it('suppresses exactly one click after an internal drag ends', () => {
		const handlers = createFileListDragHandlers(() => {});

		handlers.onDragStart(0, makeDragEvent());
		handlers.onDragEnd();

		expect(handlers.consumePostDragClick()).toBe(true);
		expect(handlers.consumePostDragClick()).toBe(false);
	});
});
