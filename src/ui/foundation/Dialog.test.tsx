import { cleanup, fireEvent, render, screen } from '@solidjs/testing-library';
import userEvent from '@testing-library/user-event';
import { createSignal, flush } from 'solid-js';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { Dialog } from './Dialog';

describe('Dialog', () => {
	afterEach(() => cleanup());

	it('marks itself open and calls onClose from the backdrop', () => {
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
		expect(screen.getByRole('dialog', { name: 'Title' })).toHaveAttribute('data-state', 'open');
		screen.getByRole('button', { name: 'Close dialog' }).click();
		expect(onClose).toHaveBeenCalledTimes(1);
		screen.getByRole('dialog', { name: 'Title' }).click();
		expect(onClose).toHaveBeenCalledTimes(1);
	});

	it('closes from Enter or Space on the dismiss control and ignores those keys inside the dialog', async () => {
		const user = userEvent.setup();
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
		const dismiss = screen.getByRole('button', { name: 'Close dialog' });
		dismiss.focus();
		await user.keyboard('{Enter}');
		await user.keyboard(' ');
		expect(onClose).toHaveBeenCalledTimes(2);
		fireEvent.keyDown(screen.getByRole('dialog', { name: 'Title' }), { key: 'Enter' });
		fireEvent.keyDown(screen.getByRole('dialog', { name: 'Title' }), { key: ' ' });
		expect(onClose).toHaveBeenCalledTimes(2);
	});

	it('calls onClose on Escape after opening post-mount, with focus outside the dialog', () => {
		const onClose = vi.fn();
		const [open, setOpen] = createSignal(false);
		render(() => (
			<Dialog open={open()} onClose={onClose} labelledBy="title">
				<Dialog.Body>Body</Dialog.Body>
			</Dialog>
		));
		setOpen(true);
		flush();
		fireEvent.keyDown(document.body, { key: 'Escape' });
		expect(onClose).toHaveBeenCalledTimes(1);

		setOpen(false);
		flush();
		fireEvent.keyDown(document.body, { key: 'Escape' });
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
