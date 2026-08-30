import { beforeEach, describe, expect, it, vi } from 'vitest';
import { createFileListPointerReorder, type FileListRowHit } from '../pointerReorder';

const POINTER_ID = 7;
const reorderFiles = vi.fn();

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

function createHandlers(
	setDragState: (state: {
		draggedIndex: number | null;
		hoveredIndex: number | null;
		hoveredEdge: 'top' | 'bottom' | null;
	}) => void = () => {},
	hitTest: (clientX: number, clientY: number) => FileListRowHit | null = () => ({
		index: 1,
		edge: 'bottom',
	}),
	isBlocked = () => false,
) {
	return createFileListPointerReorder({
		setDragState,
		hitTest,
		isBlocked,
		fileCount: () => 5,
		onReorder: (fromIndex, toIndex) => reorderFiles(fromIndex, toIndex),
	});
}

describe('createFileListPointerReorder', () => {
	beforeEach(() => {
		reorderFiles.mockClear();
	});

	it('engages only after the movement threshold', () => {
		const states: Array<{
			draggedIndex: number | null;
			hoveredIndex: number | null;
			hoveredEdge: 'top' | 'bottom' | null;
		}> = [];
		const handlers = createHandlers(
			(state) => states.push(state),
			() => ({
				index: 1,
				edge: 'bottom',
			}),
		);

		handlers.onGripPointerDown(0, gripPointerDown());
		firePointer('pointermove', { clientX: 0, clientY: 2 });
		expect(states).toHaveLength(0);

		firePointer('pointermove', { clientX: 0, clientY: 24 });
		expect(states[states.length - 1]).toEqual({
			draggedIndex: 0,
			hoveredIndex: 1,
			hoveredEdge: 'bottom',
		});

		firePointer('pointerup', {});
		expect(reorderFiles).toHaveBeenCalledWith(0, 1);
		expect(handlers.consumePostDragClick()).toBe(true);
		expect(handlers.consumePostDragClick()).toBe(false);
		handlers.dispose();
	});

	it('treats a press without threshold movement as a plain click', () => {
		const handlers = createHandlers();
		handlers.onGripPointerDown(0, gripPointerDown());
		firePointer('pointermove', { clientX: 1, clientY: 1 });
		firePointer('pointerup', {});
		expect(reorderFiles).not.toHaveBeenCalled();
		expect(handlers.consumePostDragClick()).toBe(false);
		handlers.dispose();
	});

	it('abandons the drag without reordering or click suppression on pointercancel', () => {
		const states: Array<{
			draggedIndex: number | null;
			hoveredIndex: number | null;
			hoveredEdge: 'top' | 'bottom' | null;
		}> = [];
		const handlers = createHandlers(
			(state) => states.push(state),
			() => ({
				index: 1,
				edge: 'top',
			}),
		);
		handlers.onGripPointerDown(0, gripPointerDown());
		firePointer('pointermove', { clientX: 0, clientY: 24 });
		firePointer('pointercancel', {});
		expect(reorderFiles).not.toHaveBeenCalled();
		expect(states[states.length - 1]).toEqual({
			draggedIndex: null,
			hoveredIndex: null,
			hoveredEdge: null,
		});
		expect(handlers.consumePostDragClick()).toBe(false);
		handlers.dispose();
	});

	it('ignores secondary-button presses and locked order', () => {
		const handlers = createHandlers();
		handlers.onGripPointerDown(0, {
			...gripPointerDown(),
			button: 2,
		} as unknown as PointerEvent);
		firePointer('pointermove', { clientX: 0, clientY: 24 });
		firePointer('pointerup', {});
		expect(reorderFiles).not.toHaveBeenCalled();

		const locked = createHandlers(
			() => {},
			() => ({ index: 1, edge: 'top' }),
			() => true,
		);
		locked.onGripPointerDown(0, gripPointerDown());
		firePointer('pointermove', { clientX: 0, clientY: 24 });
		firePointer('pointerup', {});
		expect(reorderFiles).not.toHaveBeenCalled();
		handlers.dispose();
		locked.dispose();
	});

	it('drops the hover target when the hit test misses or points at the dragged row', () => {
		const states: Array<{
			draggedIndex: number | null;
			hoveredIndex: number | null;
			hoveredEdge: 'top' | 'bottom' | null;
		}> = [];
		let target: FileListRowHit | null = { index: 0, edge: 'top' };
		const handlers = createHandlers(
			(state) => states.push(state),
			() => target,
		);
		handlers.onGripPointerDown(0, gripPointerDown());
		firePointer('pointermove', { clientX: 0, clientY: 24 });
		expect(states[states.length - 1]).toEqual({
			draggedIndex: 0,
			hoveredIndex: null,
			hoveredEdge: null,
		});
		target = null;
		firePointer('pointermove', { clientX: 0, clientY: 30 });
		expect(states[states.length - 1]).toEqual({
			draggedIndex: 0,
			hoveredIndex: null,
			hoveredEdge: null,
		});
		firePointer('pointerup', {});
		expect(reorderFiles).not.toHaveBeenCalled();
		handlers.dispose();
	});

	describe('midpoint hit-testing', () => {
		function reorderWithHit(from: number, hit: FileListRowHit): void {
			const handlers = createHandlers(
				() => {},
				() => hit,
			);
			handlers.onGripPointerDown(from, gripPointerDown());
			firePointer('pointermove', { clientX: 0, clientY: 24 });
			firePointer('pointerup', {});
			handlers.dispose();
		}

		it('upper half of a row inserts above it', () => {
			reorderWithHit(0, { index: 3, edge: 'top' });
			expect(reorderFiles).toHaveBeenCalledWith(0, 2);
		});

		it('lower half of a row inserts below it', () => {
			reorderWithHit(0, { index: 3, edge: 'bottom' });
			expect(reorderFiles).toHaveBeenCalledWith(0, 3);
		});

		it('lower half of the last row appends at the tail', () => {
			reorderWithHit(0, { index: 4, edge: 'bottom' });
			expect(reorderFiles).toHaveBeenCalledWith(0, 4);
		});

		it('dragging downward past a row still compensates correctly', () => {
			reorderWithHit(4, { index: 1, edge: 'top' });
			expect(reorderFiles).toHaveBeenCalledWith(4, 1);
		});

		it('no-ops when the compensated destination equals the source', () => {
			reorderWithHit(1, { index: 0, edge: 'bottom' });
			expect(reorderFiles).not.toHaveBeenCalled();
		});
	});

	describe('post-drag click suppression', () => {
		it('disarms on the next global pointerdown when no row click consumes it', () => {
			const handlers = createHandlers();
			handlers.onGripPointerDown(0, gripPointerDown());
			firePointer('pointermove', { clientX: 0, clientY: 24 });
			firePointer('pointerup', {});
			window.dispatchEvent(new PointerEvent('pointerdown', { pointerId: POINTER_ID + 1 }));
			expect(handlers.consumePostDragClick()).toBe(false);
			handlers.dispose();
		});

		it('still lets the immediate post-drag row click consume the flag', () => {
			const handlers = createHandlers();
			handlers.onGripPointerDown(0, gripPointerDown());
			firePointer('pointermove', { clientX: 0, clientY: 24 });
			firePointer('pointerup', {});
			expect(handlers.consumePostDragClick()).toBe(true);
			expect(handlers.consumePostDragClick()).toBe(false);
			handlers.dispose();
		});
	});

	describe('lostpointercapture listener leak', () => {
		it('removes the lostpointercapture listener on reset', () => {
			const states: Array<{ draggedIndex: number | null }> = [];
			const handlers = createHandlers(
				(state) => states.push(state),
				() => ({
					index: 1,
					edge: 'top',
				}),
			);
			const firstGrip = document.createElement('span');
			Object.assign(firstGrip, { setPointerCapture: vi.fn() });
			handlers.onGripPointerDown(0, {
				pointerId: POINTER_ID,
				button: 0,
				clientX: 0,
				clientY: 0,
				preventDefault: vi.fn(),
				currentTarget: firstGrip,
			} as unknown as PointerEvent);
			firePointer('pointermove', { clientX: 0, clientY: 24 });
			firePointer('pointerup', {});
			reorderFiles.mockClear();

			const secondGrip = document.createElement('span');
			Object.assign(secondGrip, { setPointerCapture: vi.fn() });
			handlers.onGripPointerDown(2, {
				pointerId: POINTER_ID,
				button: 0,
				clientX: 0,
				clientY: 0,
				preventDefault: vi.fn(),
				currentTarget: secondGrip,
			} as unknown as PointerEvent);
			firePointer('pointermove', { clientX: 0, clientY: 24 });
			firstGrip.dispatchEvent(new PointerEvent('lostpointercapture', { pointerId: POINTER_ID }));
			expect(states[states.length - 1]?.draggedIndex).toBe(2);
			firePointer('pointerup', {});
			expect(reorderFiles).toHaveBeenCalledWith(2, 1);
			handlers.dispose();
		});
	});
});
