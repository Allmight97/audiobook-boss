import { beforeEach, describe, expect, it, vi } from 'vitest';
import { createFileListPointerReorder, type FileListRowHit } from '../events';
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
			files: [{ path: '/a' }, { path: '/b' }, { path: '/c' }, { path: '/d' }, { path: '/e' }],
		});
	});

	it('engages only after the movement threshold', () => {
		const states: Array<{
			draggedIndex: number | null;
			hoveredIndex: number | null;
			hoveredEdge: 'top' | 'bottom' | null;
		}> = [];
		const handlers = createFileListPointerReorder(
			(state) => states.push(state),
			() => ({ index: 1, edge: 'bottom' }),
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
	});

	it('treats a press without threshold movement as a plain click', () => {
		const handlers = createFileListPointerReorder(
			() => {},
			() => ({ index: 1, edge: 'top' }),
		);

		handlers.onGripPointerDown(0, gripPointerDown());
		firePointer('pointermove', { clientX: 1, clientY: 1 });
		firePointer('pointerup', {});

		expect(reorderFiles).not.toHaveBeenCalled();
		expect(handlers.consumePostDragClick()).toBe(false);
	});

	it('abandons the drag without reordering or click suppression on pointercancel', () => {
		const states: Array<{
			draggedIndex: number | null;
			hoveredIndex: number | null;
			hoveredEdge: 'top' | 'bottom' | null;
		}> = [];
		const handlers = createFileListPointerReorder(
			(state) => states.push(state),
			() => ({ index: 1, edge: 'top' }),
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
		// Cancelled pointers emit no click; suppression must not eat the next one.
		expect(handlers.consumePostDragClick()).toBe(false);
	});

	it('ignores secondary-button presses and locked order', () => {
		const handlers = createFileListPointerReorder(
			() => {},
			() => ({ index: 1, edge: 'top' }),
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
		const states: Array<{
			draggedIndex: number | null;
			hoveredIndex: number | null;
			hoveredEdge: 'top' | 'bottom' | null;
		}> = [];
		let target: FileListRowHit | null = { index: 0, edge: 'top' };
		const handlers = createFileListPointerReorder(
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
	});

	describe('midpoint hit-testing (F1)', () => {
		function reorderWithHit(from: number, hit: FileListRowHit): void {
			const handlers = createFileListPointerReorder(
				() => {},
				() => hit,
			);
			handlers.onGripPointerDown(from, gripPointerDown());
			firePointer('pointermove', { clientX: 0, clientY: 24 });
			firePointer('pointerup', {});
		}

		it('upper half of a row inserts above it: [A,B,C,D,E] drag A to upper-half D -> (0,2)', () => {
			reorderWithHit(0, { index: 3, edge: 'top' });
			expect(reorderFiles).toHaveBeenCalledWith(0, 2);
		});

		it('lower half of a row inserts below it: [A,B,C,D,E] drag A to lower-half D -> (0,3)', () => {
			reorderWithHit(0, { index: 3, edge: 'bottom' });
			expect(reorderFiles).toHaveBeenCalledWith(0, 3);
		});

		it('lower half of the last row appends at the tail: drag A to lower-half E -> (0,4)', () => {
			reorderWithHit(0, { index: 4, edge: 'bottom' });
			expect(reorderFiles).toHaveBeenCalledWith(0, 4);
		});

		it('dragging downward past a row still compensates correctly: drag E to upper-half B -> (4,1)', () => {
			reorderWithHit(4, { index: 1, edge: 'top' });
			expect(reorderFiles).toHaveBeenCalledWith(4, 1);
		});

		it('no-ops when the compensated destination equals the source', () => {
			// Dropping on the lower half of the row directly above the dragged one
			// resolves to the same position it already occupies.
			reorderWithHit(1, { index: 0, edge: 'bottom' });
			expect(reorderFiles).not.toHaveBeenCalled();
		});
	});

	describe('stuck click-suppression flag (F18a)', () => {
		it('disarms on the next global pointerdown when no row click ever consumes it', () => {
			const handlers = createFileListPointerReorder(
				() => {},
				() => ({ index: 1, edge: 'top' }),
			);

			handlers.onGripPointerDown(0, gripPointerDown());
			firePointer('pointermove', { clientX: 0, clientY: 24 });
			firePointer('pointerup', {});

			// The click that would normally consume it never reaches a row (e.g.
			// the pointer released outside any row); a later unrelated pointerdown
			// must still clear the stale flag before its own click is evaluated.
			window.dispatchEvent(new PointerEvent('pointerdown', { pointerId: POINTER_ID + 1 }));

			expect(handlers.consumePostDragClick()).toBe(false);
		});

		it('still lets the immediate post-drag row click consume the flag', () => {
			const handlers = createFileListPointerReorder(
				() => {},
				() => ({ index: 1, edge: 'top' }),
			);

			handlers.onGripPointerDown(0, gripPointerDown());
			firePointer('pointermove', { clientX: 0, clientY: 24 });
			firePointer('pointerup', {});

			// The pointerup that armed suppression is not itself a pointerdown, so
			// the very next row click (calling consumePostDragClick synchronously)
			// still observes the flag armed.
			expect(handlers.consumePostDragClick()).toBe(true);
			expect(handlers.consumePostDragClick()).toBe(false);
		});
	});

	describe('lostpointercapture listener leak (F18b)', () => {
		it('removes the lostpointercapture listener on reset so a stale, delayed event cannot cancel a later drag with a reused pointer id', () => {
			const states: Array<{ draggedIndex: number | null }> = [];
			const handlers = createFileListPointerReorder(
				(state) => states.push(state),
				() => ({ index: 1, edge: 'top' }),
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
			vi.mocked(reorderFiles).mockClear();

			// Start a second drag reusing the same pointer id (mouse pointer ids
			// are recycled), then let the first grip's stale lostpointercapture
			// arrive late.
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

			// The stray event must not have cancelled the in-progress second drag.
			expect(states[states.length - 1]?.draggedIndex).toBe(2);

			firePointer('pointerup', {});
			expect(reorderFiles).toHaveBeenCalledWith(2, 1);
		});
	});
});
