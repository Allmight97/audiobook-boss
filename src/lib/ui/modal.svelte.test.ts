import { afterEach, describe, expect, it, vi } from 'vitest';
import { ModalController as BareModalController } from './modal.svelte';

async function flushMicrotasks(): Promise<void> {
	await new Promise<void>((resolve) => queueMicrotask(resolve));
}

// Track every controller so afterEach can detach the document-level listener
// of any modal a test left open — leaked listeners bleed across tests.
const liveControllers: BareModalController[] = [];
class ModalController extends BareModalController {
	constructor() {
		super();
		liveControllers.push(this);
	}
}

function dialogWithFields(): { container: HTMLElement; first: HTMLElement; last: HTMLElement } {
	const container = document.createElement('div');
	const first = document.createElement('input');
	const middle = document.createElement('input');
	const last = document.createElement('button');
	container.append(first, middle, last);
	document.body.append(container);
	return { container, first, last };
}

afterEach(() => {
	for (const controller of liveControllers.splice(0)) controller.destroy();
	document.body.innerHTML = '';
});

describe('ModalController', () => {
	it('moves initial focus to the first focusable element on open', async () => {
		const { container, first } = dialogWithFields();
		const modal = new ModalController();

		modal.sync(true, { container }, { onEscape: vi.fn() });
		await flushMicrotasks();

		expect(document.activeElement).toBe(first);
	});

	it('prefers an autofocus-designated element over the first focusable', async () => {
		const { container } = dialogWithFields();
		const autofocusTarget = document.createElement('input');
		autofocusTarget.setAttribute('autofocus', '');
		container.append(autofocusTarget);
		const modal = new ModalController();

		modal.sync(true, { container }, { onEscape: vi.fn() });
		await flushMicrotasks();

		expect(document.activeElement).toBe(autofocusTarget);
	});

	it('remembers the invoker and restores focus to it on close', async () => {
		const invoker = document.createElement('button');
		document.body.append(invoker);
		invoker.focus();
		const { container } = dialogWithFields();
		const modal = new ModalController();

		modal.sync(true, { container }, { onEscape: vi.fn() });
		await flushMicrotasks();
		expect(document.activeElement).not.toBe(invoker);

		modal.sync(false, { container }, { onEscape: vi.fn() });
		await flushMicrotasks();

		expect(document.activeElement).toBe(invoker);
	});

	it('does not restore focus to an invoker that left the document', async () => {
		const invoker = document.createElement('button');
		document.body.append(invoker);
		invoker.focus();
		const { container } = dialogWithFields();
		const modal = new ModalController();

		modal.sync(true, { container }, { onEscape: vi.fn() });
		await flushMicrotasks();
		invoker.remove();

		modal.sync(false, { container }, { onEscape: vi.fn() });
		await flushMicrotasks();

		expect(document.activeElement).not.toBe(invoker);
	});

	it('hears Escape at the document level even when focus is outside the container', async () => {
		const { container } = dialogWithFields();
		const outside = document.createElement('button');
		document.body.append(outside);
		const onEscape = vi.fn();
		const modal = new ModalController();
		modal.sync(true, { container }, { onEscape });
		await flushMicrotasks();
		outside.focus();

		outside.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
		expect(onEscape).toHaveBeenCalledTimes(1);

		// After close, the document listener is gone.
		modal.sync(false, { container }, { onEscape });
		outside.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
		expect(onEscape).toHaveBeenCalledTimes(1);
		outside.remove();
	});

	it('pulls a Tab that starts outside the open modal back to the first focusable', async () => {
		const { container, first } = dialogWithFields();
		const outside = document.createElement('button');
		document.body.append(outside);
		const modal = new ModalController();
		modal.sync(true, { container }, { onEscape: vi.fn() });
		await flushMicrotasks();
		outside.focus();

		const event = new KeyboardEvent('keydown', { key: 'Tab', bubbles: true, cancelable: true });
		outside.dispatchEvent(event);

		expect(event.defaultPrevented).toBe(true);
		expect(document.activeElement).toBe(first);
		outside.remove();
	});

	it('only the topmost of two open modals responds to Escape', async () => {
		const lower = dialogWithFields();
		const upper = dialogWithFields();
		const onLowerEscape = vi.fn();
		const onUpperEscape = vi.fn();
		const lowerModal = new ModalController();
		const upperModal = new ModalController();
		lowerModal.sync(true, { container: lower.container }, { onEscape: onLowerEscape });
		upperModal.sync(true, { container: upper.container }, { onEscape: onUpperEscape });
		await flushMicrotasks();

		document.body.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
		expect(onUpperEscape).toHaveBeenCalledTimes(1);
		expect(onLowerEscape).not.toHaveBeenCalled();

		// With the upper modal closed, the lower one becomes topmost.
		upperModal.sync(false, { container: upper.container }, { onEscape: onUpperEscape });
		document.body.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
		expect(onUpperEscape).toHaveBeenCalledTimes(1);
		expect(onLowerEscape).toHaveBeenCalledTimes(1);
	});

	it('retries initial focus on the next animation frame when the first attempt is refused', async () => {
		const { container, first } = dialogWithFields();
		const modal = new ModalController();
		// Simulate WebKit refusing focus mid-visibility-transition: the first
		// focus() call is a no-op, later calls behave normally.
		const nativeFocus = HTMLElement.prototype.focus;
		let refusals = 1;
		const focusSpy = vi.spyOn(HTMLElement.prototype, 'focus').mockImplementation(function (
			this: HTMLElement,
			...args
		) {
			if (refusals > 0) {
				refusals -= 1;
				return;
			}
			nativeFocus.apply(this, args);
		});

		modal.sync(true, { container }, { onEscape: vi.fn() });
		await flushMicrotasks();
		expect(container.contains(document.activeElement)).toBe(false);

		await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
		expect(document.activeElement).toBe(first);
		focusSpy.mockRestore();
	});

	it('routes Escape through the provided callback rather than closing itself', async () => {
		const { container } = dialogWithFields();
		const onEscape = vi.fn();
		const modal = new ModalController();
		modal.sync(true, { container }, { onEscape });

		const event = new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true });
		container.dispatchEvent(event);

		expect(onEscape).toHaveBeenCalledTimes(1);
		expect(event.defaultPrevented).toBe(true);
	});

	it('delegates to a guarded callback: Escape still calls it even when the callback itself no-ops', async () => {
		const { container } = dialogWithFields();
		let cancelDisabled = true;
		const guardedClose = vi.fn(() => {
			if (cancelDisabled) return;
		});
		const modal = new ModalController();
		modal.sync(true, { container }, { onEscape: guardedClose });

		container.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
		expect(guardedClose).toHaveBeenCalledTimes(1);

		// Matches what clicking a disabled Cancel affordance would do: no-op,
		// because the primitive never bypasses the caller's own guard.
		cancelDisabled = false;
		container.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
		expect(guardedClose).toHaveBeenCalledTimes(2);
	});

	it('wraps Tab from the last focusable element to the first', async () => {
		const { container, first, last } = dialogWithFields();
		const modal = new ModalController();
		modal.sync(true, { container }, { onEscape: vi.fn() });
		await flushMicrotasks();
		last.focus();

		const event = new KeyboardEvent('keydown', { key: 'Tab', bubbles: true, cancelable: true });
		container.dispatchEvent(event);

		expect(event.defaultPrevented).toBe(true);
		expect(document.activeElement).toBe(first);
	});

	it('wraps Shift+Tab from the first focusable element to the last', async () => {
		const { container, first, last } = dialogWithFields();
		const modal = new ModalController();
		modal.sync(true, { container }, { onEscape: vi.fn() });
		await flushMicrotasks();
		first.focus();

		const event = new KeyboardEvent('keydown', {
			key: 'Tab',
			shiftKey: true,
			bubbles: true,
			cancelable: true,
		});
		container.dispatchEvent(event);

		expect(event.defaultPrevented).toBe(true);
		expect(document.activeElement).toBe(last);
	});

	it('does not trap Tab while closed', async () => {
		const { container } = dialogWithFields();
		const modal = new ModalController();
		modal.sync(true, { container }, { onEscape: vi.fn() });
		await flushMicrotasks();
		modal.sync(false, { container }, { onEscape: vi.fn() });
		await flushMicrotasks();

		const event = new KeyboardEvent('keydown', { key: 'Tab', bubbles: true, cancelable: true });
		container.dispatchEvent(event);

		expect(event.defaultPrevented).toBe(false);
	});

	it('tolerates repeated open/close cycles, re-trapping and re-focusing each time', async () => {
		const { container, first } = dialogWithFields();
		const onEscape = vi.fn();
		const modal = new ModalController();

		modal.sync(true, { container }, { onEscape });
		await flushMicrotasks();
		modal.sync(false, { container }, { onEscape });
		await flushMicrotasks();
		modal.sync(true, { container }, { onEscape });
		await flushMicrotasks();

		expect(document.activeElement).toBe(first);
		container.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
		expect(onEscape).toHaveBeenCalledTimes(1);
	});
});
