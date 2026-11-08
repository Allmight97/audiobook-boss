type DropEdge = 'top' | 'bottom';

export type FileListDragState = {
	draggedKey: string | null;
	hoveredKey: string | null;
	hoveredEdge: DropEdge | null;
};

export type FileListRowHit = { key: string; edge: DropEdge };
export type FileListRowHitTest = (clientX: number, clientY: number) => FileListRowHit | null;
export type FileListPointerReorderHandlers = {
	onGripPointerDown: (key: string, event: PointerEvent) => void;
	consumePostDragClick: () => boolean;
	dispose: () => void;
};

type Gesture = {
	readonly key: string;
	readonly pointerId: number;
	readonly startX: number;
	readonly startY: number;
	engaged: boolean;
	hoveredKey: string | null;
	hoveredEdge: DropEdge | null;
};

const IDLE_DRAG_STATE: FileListDragState = {
	draggedKey: null,
	hoveredKey: null,
	hoveredEdge: null,
};

const REORDER_DRAG_THRESHOLD_PX = 4;

function fileListRowHitFromPoint(clientX: number, clientY: number): FileListRowHit | null {
	if (typeof document.elementFromPoint !== 'function') return null;
	const row = document.elementFromPoint(clientX, clientY)?.closest<HTMLElement>('[data-file-key]');
	if (!row?.dataset.fileKey) return null;
	const rect = row.getBoundingClientRect();
	return {
		key: row.dataset.fileKey,
		edge: clientY < rect.top + rect.height / 2 ? 'top' : 'bottom',
	};
}

export function createFileListPointerReorder(options: {
	readonly setDragState: (state: FileListDragState) => void;
	readonly hitTest?: FileListRowHitTest;
	readonly isBlocked?: () => boolean;
	readonly rowKeys: () => readonly string[];
	readonly onReorder: (fromIndex: number, toIndex: number) => void;
}): FileListPointerReorderHandlers {
	const hitTest = options.hitTest ?? fileListRowHitFromPoint;
	const isBlocked = options.isBlocked ?? (() => false);
	let gesture: Gesture | null = null;
	let capture: {
		readonly grip: Element;
		readonly listener: (event: Event) => void;
	} | null = null;
	let suppressPostDragClick = false;

	function detachWindowListeners(): void {
		window.removeEventListener('pointermove', handlePointerMove);
		window.removeEventListener('pointerup', handlePointerUp);
		window.removeEventListener('pointercancel', handlePointerCancel);
	}
	function detachLostPointerCaptureListener(): void {
		if (capture) capture.grip.removeEventListener('lostpointercapture', capture.listener);
		capture = null;
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
		gesture = null;
		detachWindowListeners();
		detachLostPointerCaptureListener();
		options.setDragState(IDLE_DRAG_STATE);
	}
	function handlePointerMove(event: PointerEvent): void {
		if (!gesture || event.pointerId !== gesture.pointerId) return;
		if (isBlocked()) {
			resetDragState();
			return;
		}
		if (!gesture.engaged) {
			const moved = Math.hypot(event.clientX - gesture.startX, event.clientY - gesture.startY);
			if (moved < REORDER_DRAG_THRESHOLD_PX) return;
			gesture.engaged = true;
		}
		const hit = hitTest(event.clientX, event.clientY);
		const hovering = hit && hit.key !== gesture.key && options.rowKeys().includes(hit.key);
		gesture.hoveredKey = hovering ? hit.key : null;
		gesture.hoveredEdge = hovering ? hit.edge : null;
		options.setDragState({
			draggedKey: gesture.key,
			hoveredKey: gesture.hoveredKey,
			hoveredEdge: gesture.hoveredEdge,
		});
	}
	function reorderOnRelease(finished: Gesture): void {
		if (finished.hoveredKey === null || finished.hoveredEdge === null || isBlocked()) return;
		const keys = options.rowKeys();
		const from = keys.indexOf(finished.key);
		const hovered = keys.indexOf(finished.hoveredKey);
		if (from < 0 || hovered < 0) return;
		const insertIndex = finished.hoveredEdge === 'top' ? hovered : hovered + 1;
		const to = from < insertIndex ? insertIndex - 1 : insertIndex;
		if (to !== from) options.onReorder(from, to);
	}
	function handlePointerUp(event: PointerEvent): void {
		if (!gesture || event.pointerId !== gesture.pointerId) return;
		const finished = gesture;
		if (finished.engaged) {
			armPostDragClickSuppression();
			reorderOnRelease(finished);
		}
		resetDragState();
	}
	function handlePointerCancel(event: PointerEvent): void {
		if (!gesture || event.pointerId !== gesture.pointerId) return;
		resetDragState();
	}
	return {
		onGripPointerDown(key, event) {
			if (event.button !== 0 || isBlocked() || !options.rowKeys().includes(key)) return;
			gesture = {
				key,
				pointerId: event.pointerId,
				startX: event.clientX,
				startY: event.clientY,
				engaged: false,
				hoveredKey: null,
				hoveredEdge: null,
			};
			event.preventDefault();
			const grip = event.currentTarget;
			if (grip instanceof Element && typeof grip.setPointerCapture === 'function') {
				try {
					grip.setPointerCapture(event.pointerId);
					const listener = (lostEvent: Event) => {
						if (lostEvent instanceof PointerEvent) handlePointerCancel(lostEvent);
					};
					capture = { grip, listener };
					grip.addEventListener('lostpointercapture', listener, { once: true });
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
