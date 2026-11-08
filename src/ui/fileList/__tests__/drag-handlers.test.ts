import { beforeEach, describe, expect, it, vi } from 'vitest';
import { createFileListPointerReorder, type FileListRowHit } from '../pointerReorder';

const POINTER_ID = 7;
const reorderFiles = vi.fn();
const ROWS = ['r0', 'r1', 'r2', 'r3', 'r4'];
const row = (index: number) => `r${index}`;

function gripPointerDown(clientX = 0, clientY = 0, button = 0): PointerEvent {
	return {
		pointerId: POINTER_ID,
		button,
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
		draggedKey: string | null;
		hoveredKey: string | null;
		hoveredEdge: 'top' | 'bottom' | null;
	}) => void = () => {},
	hitTest: (clientX: number, clientY: number) => FileListRowHit | null = () => ({
		key: 'r1',
		edge: 'bottom',
	}),
	isBlocked = () => false,
) {
	return createFileListPointerReorder({
		setDragState,
		hitTest,
		isBlocked,
		rowKeys: () => ROWS,
		onReorder: (fromIndex, toIndex) => reorderFiles(fromIndex, toIndex),
	});
}

describe('createFileListPointerReorder', () => {
	beforeEach(() => {
		reorderFiles.mockClear();
	});

	it('engages only after the movement threshold', () => {
		const states: Array<{
			draggedKey: string | null;
			hoveredKey: string | null;
			hoveredEdge: 'top' | 'bottom' | null;
		}> = [];
		const handlers = createHandlers(
			(state) => states.push(state),
			() => ({
				key: 'r1',
				edge: 'bottom',
			}),
		);

		handlers.onGripPointerDown('r0', gripPointerDown());
		firePointer('pointermove', { clientX: 0, clientY: 2 });
		expect(states).toHaveLength(0);

		firePointer('pointermove', { clientX: 0, clientY: 24 });
		expect(states[states.length - 1]).toEqual({
			draggedKey: 'r0',
			hoveredKey: 'r1',
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
		handlers.onGripPointerDown('r0', gripPointerDown());
		firePointer('pointermove', { clientX: 1, clientY: 1 });
		firePointer('pointerup', {});
		expect(reorderFiles).not.toHaveBeenCalled();
		expect(handlers.consumePostDragClick()).toBe(false);
		handlers.dispose();
	});

	it('abandons the drag without reordering or click suppression on pointercancel', () => {
		const states: Array<{
			draggedKey: string | null;
			hoveredKey: string | null;
			hoveredEdge: 'top' | 'bottom' | null;
		}> = [];
		const handlers = createHandlers(
			(state) => states.push(state),
			() => ({
				key: 'r1',
				edge: 'top',
			}),
		);
		handlers.onGripPointerDown('r0', gripPointerDown());
		firePointer('pointermove', { clientX: 0, clientY: 24 });
		firePointer('pointercancel', {});
		expect(reorderFiles).not.toHaveBeenCalled();
		expect(states[states.length - 1]).toEqual({
			draggedKey: null,
			hoveredKey: null,
			hoveredEdge: null,
		});
		expect(handlers.consumePostDragClick()).toBe(false);
		handlers.dispose();
	});

	it('ignores secondary-button presses and locked order', () => {
		const handlers = createHandlers();
		handlers.onGripPointerDown('r0', gripPointerDown(0, 0, 2));
		firePointer('pointermove', { clientX: 0, clientY: 24 });
		firePointer('pointerup', {});
		expect(reorderFiles).not.toHaveBeenCalled();

		const locked = createHandlers(
			() => {},
			() => ({ key: 'r1', edge: 'top' }),
			() => true,
		);
		locked.onGripPointerDown('r0', gripPointerDown());
		firePointer('pointermove', { clientX: 0, clientY: 24 });
		firePointer('pointerup', {});
		expect(reorderFiles).not.toHaveBeenCalled();
		handlers.dispose();
		locked.dispose();
	});

	it('drops the hover target when the hit test misses or points at the dragged row', () => {
		const states: Array<{
			draggedKey: string | null;
			hoveredKey: string | null;
			hoveredEdge: 'top' | 'bottom' | null;
		}> = [];
		let target: FileListRowHit | null = { key: 'r0', edge: 'top' };
		const handlers = createHandlers(
			(state) => states.push(state),
			() => target,
		);
		handlers.onGripPointerDown('r0', gripPointerDown());
		firePointer('pointermove', { clientX: 0, clientY: 24 });
		expect(states[states.length - 1]).toEqual({
			draggedKey: 'r0',
			hoveredKey: null,
			hoveredEdge: null,
		});
		target = null;
		firePointer('pointermove', { clientX: 0, clientY: 30 });
		expect(states[states.length - 1]).toEqual({
			draggedKey: 'r0',
			hoveredKey: null,
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
			handlers.onGripPointerDown(row(from), gripPointerDown());
			firePointer('pointermove', { clientX: 0, clientY: 24 });
			firePointer('pointerup', {});
			handlers.dispose();
		}

		it.each([
			{
				name: 'upper half inserts above',
				from: 0,
				hit: { key: 'r3', edge: 'top' },
				calls: [[0, 2]],
			},
			{
				name: 'lower half inserts below',
				from: 0,
				hit: { key: 'r3', edge: 'bottom' },
				calls: [[0, 3]],
			},
			{
				name: 'last row lower half appends',
				from: 0,
				hit: { key: 'r4', edge: 'bottom' },
				calls: [[0, 4]],
			},
			{
				name: 'upward drag compensates',
				from: 4,
				hit: { key: 'r1', edge: 'top' },
				calls: [[4, 1]],
			},
			{
				name: 'same compensated slot is a no-op',
				from: 1,
				hit: { key: 'r0', edge: 'bottom' },
				calls: [],
			},
		])('$name', ({ from, hit, calls }) => {
			reorderWithHit(from, hit as FileListRowHit);
			expect(reorderFiles.mock.calls).toEqual(calls);
		});
	});

	describe('post-drag click suppression', () => {
		it('disarms on the next global pointerdown when no row click consumes it', () => {
			const handlers = createHandlers();
			handlers.onGripPointerDown('r0', gripPointerDown());
			firePointer('pointermove', { clientX: 0, clientY: 24 });
			firePointer('pointerup', {});
			window.dispatchEvent(new PointerEvent('pointerdown', { pointerId: POINTER_ID + 1 }));
			expect(handlers.consumePostDragClick()).toBe(false);
			handlers.dispose();
		});
	});

	describe('lostpointercapture listener leak', () => {
		it('removes the lostpointercapture listener on reset', () => {
			const states: Array<{ draggedKey: string | null }> = [];
			const handlers = createHandlers(
				(state) => states.push(state),
				() => ({
					key: 'r1',
					edge: 'top',
				}),
			);
			const firstGrip = document.createElement('span');
			Object.assign(firstGrip, { setPointerCapture: vi.fn() });
			handlers.onGripPointerDown('r0', {
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
			handlers.onGripPointerDown('r2', {
				pointerId: POINTER_ID,
				button: 0,
				clientX: 0,
				clientY: 0,
				preventDefault: vi.fn(),
				currentTarget: secondGrip,
			} as unknown as PointerEvent);
			firePointer('pointermove', { clientX: 0, clientY: 24 });
			firstGrip.dispatchEvent(new PointerEvent('lostpointercapture', { pointerId: POINTER_ID }));
			expect(states[states.length - 1]?.draggedKey).toBe('r2');
			firePointer('pointerup', {});
			expect(reorderFiles).toHaveBeenCalledWith(2, 1);
			handlers.dispose();
		});
	});
});

describe('a list that shifts during the drag', () => {
	function keyedHandlers(
		rowKeys: () => readonly string[],
		hit: { key: string; edge: 'top' | 'bottom' },
	) {
		return createFileListPointerReorder({
			setDragState: () => {},
			hitTest: () => hit,
			rowKeys,
			onReorder: (fromIndex, toIndex) => reorderFiles(fromIndex, toIndex),
		});
	}

	it('moves the title that was pressed, not the row now at its old index', () => {
		reorderFiles.mockClear();
		let keys = ['a', 'b', 'c', 'd', 'e'];
		const handlers = keyedHandlers(() => keys, { key: 'd', edge: 'bottom' });

		handlers.onGripPointerDown('b', gripPointerDown());
		firePointer('pointermove', { clientX: 0, clientY: 24 });
		keys = ['x', 'a', 'b', 'c', 'd', 'e'];
		firePointer('pointerup', {});

		expect(reorderFiles).toHaveBeenCalledWith(2, 4);
		handlers.dispose();
	});

	it('drops the drag when the pressed title leaves the list', () => {
		reorderFiles.mockClear();
		let keys = ['a', 'b', 'c'];
		const handlers = keyedHandlers(() => keys, { key: 'c', edge: 'bottom' });

		handlers.onGripPointerDown('a', gripPointerDown());
		firePointer('pointermove', { clientX: 0, clientY: 24 });
		keys = ['b', 'c'];
		firePointer('pointerup', {});

		expect(reorderFiles).not.toHaveBeenCalled();
		handlers.dispose();
	});
});
