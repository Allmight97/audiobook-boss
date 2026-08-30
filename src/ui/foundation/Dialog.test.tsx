import { cleanup, render, screen } from '@solidjs/testing-library';
import { createSignal } from 'solid-js';
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
});
