import { afterEach, describe, expect, it, vi } from 'vitest';
import { ModalController } from './modal.svelte';

async function flushMicrotasks(): Promise<void> {
	await new Promise<void>((resolve) => queueMicrotask(resolve));
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
