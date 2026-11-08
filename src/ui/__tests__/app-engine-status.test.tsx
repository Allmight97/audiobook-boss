import { screen } from '@solidjs/testing-library';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { createFakeEngine } from '../../test/fixtures/fakeEngine';
import { renderApp } from '../../test/fixtures/renderApp';

describe('app around the engine', () => {
	it('shows a refused change until the user dismisses it', async () => {
		const engine = createFakeEngine();
		engine.respond = (intent) =>
			intent.kind === 'toggleSort'
				? {
						kind: 'rejected',
						error: {
							code: 'invalid_input',
							category: 'validation',
							message: 'The list is locked.',
							detail: null,
						},
					}
				: undefined;
		const { runtime } = await renderApp({ engine });

		runtime.input.toggleSort();
		expect(await screen.findByText('The list is locked.')).toBeInTheDocument();
		await userEvent.click(screen.getByRole('button', { name: 'Dismiss' }));
		expect(screen.queryByText('The list is locked.')).toBeNull();
	});

	it('reminds the user to press Quit again, then clears when the window ends', async () => {
		const engine = createFakeEngine();
		await renderApp({ engine });

		engine.pressQuitOnce(20);

		expect(await screen.findByText('Press ⌘Q again to quit')).toBeInTheDocument();
		await vi.waitFor(() => expect(screen.queryByText('Press ⌘Q again to quit')).toBeNull());
	});
});
