export type PopoverElements = {
	anchor?: HTMLElement | null;
	container?: HTMLElement | null;
	panel?: HTMLElement | null;
	clickBoundary?: HTMLElement | null;
};

export type PopoverPosition = {
	left: number;
	top: number;
};

export type PopoverControllerOptions = {
	gap?: number;
	inset?: number;
	closeOnClickAway?: boolean;
	onOpenChange?: (open: boolean) => void;
};

export type PopoverOpenOptions = {
	focusInside?: boolean;
};

const FOCUSABLE_SELECTOR =
	'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

function clamp(value: number, minimum: number, maximum: number): number {
	return Math.min(Math.max(value, minimum), Math.max(minimum, maximum));
}

export function calculatePopoverPosition(
	anchor: DOMRect,
	container: DOMRect,
	panel: DOMRect,
	options: Pick<PopoverControllerOptions, 'gap' | 'inset'> = {},
): PopoverPosition {
	const gap = options.gap ?? 6;
	const inset = options.inset ?? 8;
	return {
		left: clamp(anchor.left - container.left, inset, container.width - panel.width - inset),
		top: clamp(anchor.bottom - container.top + gap, inset, container.height - panel.height - inset),
	};
}

export class PopoverController {
	isOpen = $state(false);
	position = $state<PopoverPosition>({ left: 0, top: 0 });

	#anchor: HTMLElement | null = null;
	#container: HTMLElement | null = null;
	#panel: HTMLElement | null = null;
	#clickBoundary: HTMLElement | null = null;
	#focusInsideOnOpen = true;
	#options: PopoverControllerOptions;

	constructor(options: PopoverControllerOptions = {}) {
		this.#options = options;
	}

	setElements(elements: PopoverElements): void {
		if (elements.anchor !== undefined) this.#anchor = elements.anchor;
		if (elements.container !== undefined) this.#container = elements.container;
		if (elements.panel !== undefined) this.#panel = elements.panel;
		if (elements.clickBoundary !== undefined) this.#clickBoundary = elements.clickBoundary;
		if (this.isOpen) {
			this.reposition();
			if (elements.panel && this.#focusInsideOnOpen) this.#focusInside();
		}
	}

	open(options: PopoverOpenOptions = {}): void {
		if (this.isOpen) return;
		this.#focusInsideOnOpen = options.focusInside ?? true;
		this.isOpen = true;
		this.reposition();
		if (this.#focusInsideOnOpen) this.#focusInside();
		this.#options.onOpenChange?.(true);
	}

	close(options: { restoreFocus?: boolean } = {}): void {
		if (!this.isOpen) return;
		this.isOpen = false;
		this.#options.onOpenChange?.(false);
		if (options.restoreFocus ?? true) {
			const anchor = this.#anchor;
			queueMicrotask(() => anchor?.focus());
		}
	}

	toggle(options?: PopoverOpenOptions): void {
		if (this.isOpen) this.close();
		else this.open(options);
	}

	reposition(): void {
		if (!this.#anchor || !this.#container || !this.#panel) return;
		this.position = calculatePopoverPosition(
			this.#anchor.getBoundingClientRect(),
			this.#container.getBoundingClientRect(),
			this.#panel.getBoundingClientRect(),
			this.#options,
		);
	}

	handleKeydown(event: KeyboardEvent): void {
		if (event.key !== 'Escape' || !this.isOpen) return;
		event.preventDefault();
		this.close();
	}

	handleClickAway(event: MouseEvent): void {
		if (!this.isOpen || !this.#options.closeOnClickAway) return;
		const target = event.target;
		if (!(target instanceof Node) || this.#clickBoundary?.contains(target)) return;
		this.close({ restoreFocus: false });
	}

	#focusInside(): void {
		const panel = this.#panel;
		if (!panel) return;
		queueMicrotask(() => {
			if (!this.isOpen || panel !== this.#panel) return;
			(panel.querySelector<HTMLElement>(FOCUSABLE_SELECTOR) ?? panel).focus();
		});
	}
}
