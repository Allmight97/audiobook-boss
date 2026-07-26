import { afterEach, describe, expect, it, vi } from 'vitest';
import { PopoverController } from './popover.svelte';

function rect(left: number, top: number, width: number, height: number): DOMRect {
	return {
		left,
		top,
		width,
		height,
		right: left + width,
		bottom: top + height,
		x: left,
		y: top,
		toJSON: () => ({}),
	};
}

function mockRect(element: HTMLElement, value: DOMRect): void {
	element.getBoundingClientRect = vi.fn(() => value);
}

async function flushMicrotasks(): Promise<void> {
	await new Promise<void>((resolve) => queueMicrotask(resolve));
}

afterEach(() => {
	document.body.innerHTML = '';
});

describe('PopoverController', () => {
	it('measures the elements and clamps the position inside the container', () => {
		const anchor = document.createElement('button');
		const container = document.createElement('div');
		const panel = document.createElement('div');
		mockRect(anchor, rect(440, 260, 40, 24));
		mockRect(container, rect(100, 50, 400, 300));
		mockRect(panel, rect(0, 0, 180, 140));

		const popover = new PopoverController();
		popover.setElements({ anchor, container, panel });
		popover.open();

		expect(popover.position).toEqual({ left: 212, top: 152 });
		expect(anchor.getBoundingClientRect).toHaveBeenCalled();
		expect(container.getBoundingClientRect).toHaveBeenCalled();
		expect(panel.getBoundingClientRect).toHaveBeenCalled();
	});

	it('closes when Escape is pressed', () => {
		const popover = new PopoverController();
		popover.open();
		const event = new KeyboardEvent('keydown', { key: 'Escape', cancelable: true });

		popover.handleKeydown(event);

		expect(popover.isOpen).toBe(false);
		expect(event.defaultPrevented).toBe(true);
	});

	it('moves initial focus inside when opened', async () => {
		const anchor = document.createElement('button');
		const container = document.createElement('div');
		const panel = document.createElement('div');
		const input = document.createElement('input');
		panel.append(input);
		document.body.append(anchor, container, panel);

		const popover = new PopoverController();
		popover.setElements({ anchor, container, panel });
		popover.open();
		await flushMicrotasks();

		expect(document.activeElement).toBe(input);
	});

	it('restores focus to the anchor when closed', async () => {
		const anchor = document.createElement('button');
		const container = document.createElement('div');
		const panel = document.createElement('div');
		const input = document.createElement('input');
		panel.append(input);
		document.body.append(anchor, container, panel);

		const popover = new PopoverController();
		popover.setElements({ anchor, container, panel });
		popover.open();
		await flushMicrotasks();
		popover.close();
		await flushMicrotasks();

		expect(document.activeElement).toBe(anchor);
	});

	it('preserves click-away focus when closed without restoration', async () => {
		const anchor = document.createElement('button');
		const target = document.createElement('button');
		document.body.append(anchor, target);
		const popover = new PopoverController();
		popover.setElements({ anchor });
		popover.open();
		target.focus();
		popover.close({ restoreFocus: false });
		await flushMicrotasks();

		expect(document.activeElement).toBe(target);
	});

	it('leaves click-away disabled unless the controller opts in', () => {
		const boundary = document.createElement('div');
		const target = document.createElement('button');
		document.body.append(boundary, target);
		const popover = new PopoverController();
		popover.setElements({ clickBoundary: boundary });
		popover.open({ focusInside: false });

		const event = new MouseEvent('click', { bubbles: true });
		target.dispatchEvent(event);
		popover.handleClickAway(event);

		expect(popover.isOpen).toBe(true);
	});

	it('closes opted-in menus on click-away without restoring focus', async () => {
		const anchor = document.createElement('button');
		const boundary = document.createElement('div');
		const target = document.createElement('button');
		document.body.append(anchor, boundary, target);
		const popover = new PopoverController({ closeOnClickAway: true });
		popover.setElements({ anchor, clickBoundary: boundary });
		popover.open({ focusInside: false });
		target.focus();

		const event = new MouseEvent('click', { bubbles: true });
		target.dispatchEvent(event);
		popover.handleClickAway(event);
		await flushMicrotasks();

		expect(popover.isOpen).toBe(false);
		expect(document.activeElement).toBe(target);
	});

	it('keeps opted-in menus open for clicks inside their boundary', () => {
		const boundary = document.createElement('div');
		const target = document.createElement('button');
		boundary.append(target);
		document.body.append(boundary);
		const popover = new PopoverController({ closeOnClickAway: true });
		popover.setElements({ clickBoundary: boundary });
		popover.open({ focusInside: false });

		const event = new MouseEvent('click', { bubbles: true });
		target.dispatchEvent(event);
		popover.handleClickAway(event);

		expect(popover.isOpen).toBe(true);
	});
});
