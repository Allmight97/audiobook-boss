const FOCUSABLE_SELECTOR =
	'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

export type ModalElements = {
	container?: HTMLElement | null;
};

export type ModalSyncOptions = {
	onEscape: () => void;
};

function focusableElements(container: HTMLElement): HTMLElement[] {
	return Array.from(container.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR));
}

// Open controllers in opening order; the last entry is the topmost modal and
// the only one that responds to Escape/Tab. Each controller has its own
// document listener, and stopPropagation cannot arbitrate between listeners
// on the same node — this stack can.
const openModals: ModalController[] = [];

/// Escape/focus-trap/initial-focus/focus-restore for a dialog that stays
/// MOUNTED while hidden (visibility toggled by the owning state module), so
/// behavior keys on open-state TRANSITIONS rather than component mount/destroy.
/// Call `sync` from a Svelte `$effect` that reads the owning `isOpen` state,
/// mirroring `PopoverController.setElements`. Escape always routes through the
/// caller-supplied `onEscape` callback — the same close/cancel path a visible
/// Close/Cancel button uses — never a force-hide here.
export class ModalController {
	#isOpen = false;
	#container: HTMLElement | null = null;
	#onEscape: (() => void) | null = null;
	#invoker: HTMLElement | null = null;
	#keydownHandler = (event: KeyboardEvent): void => this.#handleKeydown(event);

	sync(open: boolean, elements: ModalElements, options: ModalSyncOptions): void {
		this.#container = elements.container ?? null;
		this.#onEscape = options.onEscape;
		if (open && !this.#isOpen) {
			this.#isOpen = true;
			this.#handleOpenTransition();
		} else if (!open && this.#isOpen) {
			this.#isOpen = false;
			this.#handleCloseTransition();
		}
	}

	destroy(): void {
		document.removeEventListener('keydown', this.#keydownHandler, true);
		this.#removeFromStack();
	}

	#removeFromStack(): void {
		const index = openModals.indexOf(this);
		if (index !== -1) openModals.splice(index, 1);
	}

	#handleOpenTransition(): void {
		this.#invoker = document.activeElement instanceof HTMLElement ? document.activeElement : null;
		// Document-level, capture-phase: Escape must work wherever focus sits.
		// A container-scoped listener goes deaf whenever focus is outside the
		// dialog (initial focus refused while the backdrop's visibility
		// transition hasn't ticked, or WebKit blurring to body on a click of a
		// non-focusable area), which is exactly the live-app failure this
		// replaces.
		document.addEventListener('keydown', this.#keydownHandler, true);
		openModals.push(this);
		this.#focusInside();
	}

	#handleCloseTransition(): void {
		document.removeEventListener('keydown', this.#keydownHandler, true);
		this.#removeFromStack();
		const invoker = this.#invoker;
		this.#invoker = null;
		if (invoker && document.contains(invoker)) {
			queueMicrotask(() => invoker.focus());
		}
	}

	#focusInside(): void {
		const container = this.#container;
		if (!container) return;
		const attempt = (): boolean => {
			if (!this.#isOpen || container !== this.#container) return true;
			const target =
				container.querySelector<HTMLElement>('[autofocus]') ??
				focusableElements(container)[0] ??
				container;
			target.focus();
			return container.contains(document.activeElement);
		};
		queueMicrotask(() => {
			// WebKit refuses focus while the backdrop's visibility transition
			// hasn't had a style recalc, so retry after the next frame.
			if (!attempt() && typeof requestAnimationFrame === 'function') {
				requestAnimationFrame(() => attempt());
			}
		});
	}

	#handleKeydown(event: KeyboardEvent): void {
		if (openModals[openModals.length - 1] !== this) return;
		if (event.key === 'Escape') {
			// Capture-phase on document: consume Escape exclusively so it
			// cannot also dismiss an overlay layered behind the modal.
			event.preventDefault();
			event.stopPropagation();
			this.#onEscape?.();
			return;
		}
		if (event.key === 'Tab') {
			this.#handleTab(event);
		}
	}

	#handleTab(event: KeyboardEvent): void {
		const container = this.#container;
		if (!container) return;
		const focusable = focusableElements(container);
		if (focusable.length === 0) return;
		const first = focusable[0];
		const last = focusable[focusable.length - 1];
		const active = document.activeElement;
		if (!container.contains(active)) {
			// Focus drifted outside the open modal (blur-to-body, refused
			// initial focus): pull it back in rather than cycling relative to
			// wherever it ended up.
			event.preventDefault();
			(event.shiftKey ? last : first).focus();
			return;
		}
		if (event.shiftKey) {
			if (active === first) {
				event.preventDefault();
				last.focus();
			}
		} else if (active === last) {
			event.preventDefault();
			first.focus();
		}
	}
}
