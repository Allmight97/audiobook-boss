import { createSignal, flush } from 'solid-js';
import { cleanup, render, screen } from '@solidjs/testing-library';
import { afterEach, expect, it, vi } from 'vitest';
import { JobRow } from './JobRow';

afterEach(() => cleanup());

it('marks the row with its status and runs the injected action', () => {
	const onAction = vi.fn();
	const { container } = render(() => (
		<JobRow
			status="running"
			label="Book.m4b"
			statusText="Running"
			actions={[{ id: 'cancel', label: 'Cancel', title: 'Cancel Book.m4b only', onAction }]}
		/>
	));
	expect(container.querySelector('.abb-job-row')).toHaveClass('is-running');
	screen.getByRole('button', { name: 'Cancel' }).click();
	expect(onAction).toHaveBeenCalledTimes(1);
});

it('keeps a button across a new actions list and drops one that left it', () => {
	const [actions, setActions] = createSignal([
		{ id: 'cancel', label: 'Cancel', onAction: () => undefined },
		{ id: 'reveal', label: 'Show in Folder', onAction: () => undefined },
	]);
	render(() => <JobRow status="queued" label="Book.m4b" statusText="Queued" actions={actions()} />);
	const cancel = screen.getByRole('button', { name: 'Cancel' });

	setActions([{ id: 'cancel', label: 'Cancel', onAction: () => undefined }]);

	flush();

	expect(screen.getByRole('button', { name: 'Cancel' })).toBe(cancel);
	expect(screen.queryByRole('button', { name: 'Show in Folder' })).toBeNull();
});
