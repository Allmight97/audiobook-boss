const FOCUSABLE_SELECTOR =
	'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

export type ModalElements = {
	container?: HTMLElement | null;
};

export type ModalSyncOptions = {
	onEscape: () => void;
	restoreInvoker?: boolean;
};

function focusableElements(container: HTMLElement): HTMLElement[] {
	return Array.from(container.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR));
}

const openModalsInOpeningOrder: ModalController[] = [];

export class ModalController {
	#isOpen = false;
	#container: HTMLElement | null = null;
	#onEscape: (() => void) | null = null;
	#invoker: HTMLElement | null = null;
	#restoreInvoker = true;
	#keydownHandler = (event: KeyboardEvent): void => this.#handleKeydown(event);

	sync(open: boolean, elements: ModalElements, options: ModalSyncOptions): void {
		this.#container = elements.container ?? null;
		this.#onEscape = options.onEscape;
		this.#restoreInvoker = options.restoreInvoker !== false;
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
		const index = openModalsInOpeningOrder.indexOf(this);
		if (index !== -1) openModalsInOpeningOrder.splice(index, 1);
	}

	#handleOpenTransition(): void {
		this.#invoker = document.activeElement instanceof HTMLElement ? document.activeElement : null;
		// Document-level, capture-phase: WebKit can refuse initial focus or blur to body, which would leave a container-scoped listener deaf.
		document.addEventListener('keydown', this.#keydownHandler, true);
		openModalsInOpeningOrder.push(this);
		this.#focusInside();
	}

	#handleCloseTransition(): void {
		document.removeEventListener('keydown', this.#keydownHandler, true);
		this.#removeFromStack();
		const invoker = this.#invoker;
		this.#invoker = null;
		if (this.#restoreInvoker && invoker && document.contains(invoker)) {
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
		if (openModalsInOpeningOrder[openModalsInOpeningOrder.length - 1] !== this) return;
		if (event.key === 'Escape') {
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
