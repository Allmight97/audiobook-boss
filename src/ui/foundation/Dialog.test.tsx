import { cleanup, render, screen } from '@solidjs/testing-library';
import { createSignal, flush } from 'solid-js';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { Dialog } from './Dialog';

describe('Dialog', () => {
	afterEach(() => cleanup());

	it('toggles open class and calls onClose from the backdrop', () => {
		const onClose = vi.fn();
		const [open] = createSignal(true);
		render(() => (
			<Dialog open={open()} onClose={onClose} labelledBy="title" testId="sample-dialog">
				<Dialog.Header>
					<h3 id="title">Title</h3>
				</Dialog.Header>
				<Dialog.Body>Body</Dialog.Body>
			</Dialog>
		));
		const backdrop = screen.getByTestId('sample-dialog');
		expect(backdrop.classList.contains('open')).toBe(true);
		backdrop.click();
		expect(onClose).toHaveBeenCalledTimes(1);
	});

	function closable() {
		const [open, setOpen] = createSignal(true);
		render(() => (
			<Dialog
				open={open()}
				onClose={() => setOpen(false)}
				labelledBy="title"
				testId="sample-dialog"
				unmountWhenClosed
			>
				<Dialog.Body>Rows</Dialog.Body>
			</Dialog>
		));
		flush();
		return { setOpen, backdrop: screen.getByTestId('sample-dialog') };
	}

	it('keeps unmountWhenClosed content through the closing fade, then drops it', () => {
		vi.useFakeTimers();
		try {
			const { setOpen, backdrop } = closable();
			backdrop.style.transitionDuration = '0.18s';
			setOpen(false);
			flush();
			vi.advanceTimersByTime(100);
			flush();
			const rows = screen.getByText('Rows');
			setOpen(true);
			flush();
			expect(screen.getByText('Rows')).toBe(rows);
			setOpen(false);
			flush();
			vi.advanceTimersByTime(100);
			flush();
			expect(screen.getByText('Rows')).toBe(rows);
			vi.advanceTimersByTime(100);
			flush();
			expect(screen.queryByText('Rows')).not.toBeInTheDocument();
			setOpen(true);
			flush();
			expect(screen.getByText('Rows')).toBeInTheDocument();
		} finally {
			vi.useRealTimers();
		}
	});

	it('drops unmountWhenClosed content at once when nothing fades', async () => {
		const { setOpen } = closable();
		setOpen(false);
		flush();
		await new Promise((resolve) => setTimeout(resolve, 0));
		flush();
		expect(screen.queryByText('Rows')).not.toBeInTheDocument();
	});
});
