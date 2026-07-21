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
		this.#container?.removeEventListener('keydown', this.#keydownHandler);
	}

	#handleOpenTransition(): void {
		this.#invoker = document.activeElement instanceof HTMLElement ? document.activeElement : null;
		this.#container?.addEventListener('keydown', this.#keydownHandler);
		this.#focusInside();
	}

	#handleCloseTransition(): void {
		this.#container?.removeEventListener('keydown', this.#keydownHandler);
		const invoker = this.#invoker;
		this.#invoker = null;
		if (invoker && document.contains(invoker)) {
			queueMicrotask(() => invoker.focus());
		}
	}

	#focusInside(): void {
		const container = this.#container;
		if (!container) return;
		queueMicrotask(() => {
			if (!this.#isOpen || container !== this.#container) return;
			const target =
				container.querySelector<HTMLElement>('[autofocus]') ??
				focusableElements(container)[0] ??
				container;
			target.focus();
		});
	}

	#handleKeydown(event: KeyboardEvent): void {
		if (event.key === 'Escape') {
			event.preventDefault();
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
		if (event.shiftKey) {
			if (active === first || !container.contains(active)) {
				event.preventDefault();
				last.focus();
			}
		} else if (active === last || !container.contains(active)) {
			event.preventDefault();
			first.focus();
		}
	}
}
