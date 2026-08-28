export type FileListDragState = {
	draggedIndex: number | null;
	hoveredIndex: number | null;
	hoveredEdge: 'top' | 'bottom' | null;
};

export type FileListRowHit = { index: number; edge: 'top' | 'bottom' };
export type FileListRowHitTest = (clientX: number, clientY: number) => FileListRowHit | null;
export type FileListPointerReorderHandlers = {
	onGripPointerDown: (index: number, event: PointerEvent) => void;
	consumePostDragClick: () => boolean;
	dispose: () => void;
};

const REORDER_DRAG_THRESHOLD_PX = 4;

export function fileListRowHitFromPoint(clientX: number, clientY: number): FileListRowHit | null {
	if (typeof document.elementFromPoint !== 'function') return null;
	const row = document
		.elementFromPoint(clientX, clientY)
		?.closest<HTMLElement>('[data-file-index]');
	if (!row) return null;
	const index = Number.parseInt(row.dataset.fileIndex ?? '', 10);
	if (!Number.isInteger(index)) return null;
	const rect = row.getBoundingClientRect();
	return { index, edge: clientY < rect.top + rect.height / 2 ? 'top' : 'bottom' };
}

export function createFileListPointerReorder(options: {
	readonly setDragState: (state: FileListDragState) => void;
	readonly hitTest?: FileListRowHitTest;
	readonly isBlocked?: () => boolean;
	readonly fileCount?: () => number;
	readonly onReorder: (fromIndex: number, toIndex: number) => void;
}): FileListPointerReorderHandlers {
	const hitTest = options.hitTest ?? fileListRowHitFromPoint;
	const isBlocked = options.isBlocked ?? (() => false);
	const fileCount = options.fileCount ?? (() => 0);
	let pressedIndex: number | null = null;
	let pressedPointerId: number | null = null;
	let startX = 0;
	let startY = 0;
	let dragEngaged = false;
	let draggedIndex: number | null = null;
	let hoveredIndex: number | null = null;
	let hoveredEdge: 'top' | 'bottom' | null = null;
	let suppressPostDragClick = false;
	let pressedGrip: Element | null = null;
	let lostPointerCaptureListener: ((event: Event) => void) | null = null;

	function hasValidIndex(index: number): boolean {
		return index >= 0 && index < fileCount();
	}
	function detachWindowListeners(): void {
		window.removeEventListener('pointermove', handlePointerMove);
		window.removeEventListener('pointerup', handlePointerUp);
		window.removeEventListener('pointercancel', handlePointerCancel);
	}
	function detachLostPointerCaptureListener(): void {
		if (pressedGrip && lostPointerCaptureListener) {
			pressedGrip.removeEventListener('lostpointercapture', lostPointerCaptureListener);
		}
		pressedGrip = null;
		lostPointerCaptureListener = null;
	}
	function disarmPostDragClickSuppression(): void {
		suppressPostDragClick = false;
		window.removeEventListener('pointerdown', disarmPostDragClickSuppression, true);
	}
	function armPostDragClickSuppression(): void {
		suppressPostDragClick = true;
		window.addEventListener('pointerdown', disarmPostDragClickSuppression, true);
	}
	function resetDragState(): void {
		pressedIndex = null;
		pressedPointerId = null;
		dragEngaged = false;
		draggedIndex = null;
		hoveredIndex = null;
		hoveredEdge = null;
		detachWindowListeners();
		detachLostPointerCaptureListener();
		options.setDragState({ draggedIndex: null, hoveredIndex: null, hoveredEdge: null });
	}
	function handlePointerMove(event: PointerEvent): void {
		if (event.pointerId !== pressedPointerId || pressedIndex === null) return;
		if (isBlocked()) {
			resetDragState();
			return;
		}
		if (!dragEngaged) {
			if (Math.hypot(event.clientX - startX, event.clientY - startY) < REORDER_DRAG_THRESHOLD_PX)
				return;
			dragEngaged = true;
			draggedIndex = pressedIndex;
		}
		const hit = hitTest(event.clientX, event.clientY);
		if (hit && hit.index !== draggedIndex && hasValidIndex(hit.index)) {
			hoveredIndex = hit.index;
			hoveredEdge = hit.edge;
		} else {
			hoveredIndex = null;
			hoveredEdge = null;
		}
		options.setDragState({ draggedIndex, hoveredIndex, hoveredEdge });
	}
	function handlePointerUp(event: PointerEvent): void {
		if (event.pointerId !== pressedPointerId) return;
		if (dragEngaged) {
			armPostDragClickSuppression();
			const from = draggedIndex;
			if (from !== null && hoveredIndex !== null && hoveredEdge !== null && !isBlocked()) {
				const insertIndex = hoveredEdge === 'top' ? hoveredIndex : hoveredIndex + 1;
				const to = from < insertIndex ? insertIndex - 1 : insertIndex;
				if (to !== from) options.onReorder(from, to);
			}
		}
		resetDragState();
	}
	function handlePointerCancel(event: PointerEvent): void {
		if (event.pointerId !== pressedPointerId) return;
		resetDragState();
	}
	return {
		onGripPointerDown(index, event) {
			if (event.button !== 0 || isBlocked() || !hasValidIndex(index)) return;
			pressedIndex = index;
			pressedPointerId = event.pointerId;
			startX = event.clientX;
			startY = event.clientY;
			event.preventDefault();
			const grip = event.currentTarget;
			if (grip instanceof Element && typeof grip.setPointerCapture === 'function') {
				try {
					grip.setPointerCapture(event.pointerId);
					lostPointerCaptureListener = (lostEvent) =>
						handlePointerCancel(lostEvent as PointerEvent);
					pressedGrip = grip;
					grip.addEventListener('lostpointercapture', lostPointerCaptureListener, { once: true });
				} catch {
					// Window listeners still carry the gesture when capture is unavailable.
				}
			}
			window.addEventListener('pointermove', handlePointerMove);
			window.addEventListener('pointerup', handlePointerUp);
			window.addEventListener('pointercancel', handlePointerCancel);
		},
		consumePostDragClick() {
			const shouldSuppress = suppressPostDragClick;
			if (shouldSuppress) disarmPostDragClickSuppression();
			return shouldSuppress;
		},
		dispose() {
			disarmPostDragClickSuppression();
			resetDragState();
		},
	};
}
