import { beforeEach, describe, expect, it, vi } from 'vitest';
import { createFileListPointerReorder } from '../events';
import { reorderFiles } from '../actions';

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

const POINTER_ID = 7;

function gripPointerDown(clientX = 0, clientY = 0): PointerEvent {
	return {
		pointerId: POINTER_ID,
		button: 0,
		clientX,
		clientY,
		preventDefault: vi.fn(),
		currentTarget: document.createElement('span'),
	} as unknown as PointerEvent;
}

function firePointer(type: 'pointermove' | 'pointerup' | 'pointercancel', init: PointerEventInit) {
	window.dispatchEvent(new PointerEvent(type, { pointerId: POINTER_ID, ...init }));
}

describe('createFileListPointerReorder', () => {
	beforeEach(() => {
		vi.mocked(reorderFiles).mockClear();
		context.isOrderLockedMock.mockReturnValue(false);
		context.getCurrentFileListMock.mockReturnValue({
			files: [{ path: '/books/a.m4b' }, { path: '/books/b.m4b' }],
		});
	});

	it('engages only after the movement threshold and reorders onto the hit-tested row', () => {
		const states: Array<{ draggedIndex: number | null; hoveredIndex: number | null }> = [];
		const handlers = createFileListPointerReorder(
			(state) => states.push(state),
			() => 1,
		);

		handlers.onGripPointerDown(0, gripPointerDown());
		firePointer('pointermove', { clientX: 0, clientY: 2 });
		expect(states).toHaveLength(0);

		firePointer('pointermove', { clientX: 0, clientY: 24 });
		expect(states[states.length - 1]).toEqual({ draggedIndex: 0, hoveredIndex: 1 });

		firePointer('pointerup', {});
		expect(reorderFiles).toHaveBeenCalledWith(0, 1);
		expect(handlers.consumePostDragClick()).toBe(true);
		expect(handlers.consumePostDragClick()).toBe(false);
	});

	it('treats a press without threshold movement as a plain click', () => {
		const handlers = createFileListPointerReorder(
			() => {},
			() => 1,
		);

		handlers.onGripPointerDown(0, gripPointerDown());
		firePointer('pointermove', { clientX: 1, clientY: 1 });
		firePointer('pointerup', {});

		expect(reorderFiles).not.toHaveBeenCalled();
		expect(handlers.consumePostDragClick()).toBe(false);
	});

	it('abandons the drag without reordering on pointercancel', () => {
		const states: Array<{ draggedIndex: number | null; hoveredIndex: number | null }> = [];
		const handlers = createFileListPointerReorder(
			(state) => states.push(state),
			() => 1,
		);

		handlers.onGripPointerDown(0, gripPointerDown());
		firePointer('pointermove', { clientX: 0, clientY: 24 });
		firePointer('pointercancel', {});

		expect(reorderFiles).not.toHaveBeenCalled();
		expect(states[states.length - 1]).toEqual({ draggedIndex: null, hoveredIndex: null });
		expect(handlers.consumePostDragClick()).toBe(true);
	});

	it('ignores secondary-button presses and locked order', () => {
		const handlers = createFileListPointerReorder(
			() => {},
			() => 1,
		);

		handlers.onGripPointerDown(0, {
			...gripPointerDown(),
			button: 2,
		} as unknown as PointerEvent);
		firePointer('pointermove', { clientX: 0, clientY: 24 });
		firePointer('pointerup', {});
		expect(reorderFiles).not.toHaveBeenCalled();

		context.isOrderLockedMock.mockReturnValue(true);
		handlers.onGripPointerDown(0, gripPointerDown());
		firePointer('pointermove', { clientX: 0, clientY: 24 });
		firePointer('pointerup', {});
		expect(reorderFiles).not.toHaveBeenCalled();
	});

	it('drops the hover target when the hit test misses or points at the dragged row', () => {
		const states: Array<{ draggedIndex: number | null; hoveredIndex: number | null }> = [];
		let target: number | null = 0;
		const handlers = createFileListPointerReorder(
			(state) => states.push(state),
			() => target,
		);

		handlers.onGripPointerDown(0, gripPointerDown());
		firePointer('pointermove', { clientX: 0, clientY: 24 });
		expect(states[states.length - 1]).toEqual({ draggedIndex: 0, hoveredIndex: null });

		target = null;
		firePointer('pointermove', { clientX: 0, clientY: 30 });
		expect(states[states.length - 1]).toEqual({ draggedIndex: 0, hoveredIndex: null });

		firePointer('pointerup', {});
		expect(reorderFiles).not.toHaveBeenCalled();
	});
});
