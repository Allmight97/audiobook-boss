import { cleanup, render, screen } from '@solidjs/testing-library';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { AppRuntimeProvider, createAppRuntime, type AppRuntime } from '../../app/runtime';
import type { InputCapability } from '../../lib/tauri/capabilities/input';
import { createFakeEngine, type FakeEngine } from '../../test/fixtures/fakeEngine';
import { App } from '../App';

function fakeInput(): InputCapability {
	return {
		openFiles: vi.fn(async () => null),
		openDirectory: vi.fn(async () => null),
		listenDragDrop: vi.fn(async () => () => undefined),
		listenDragEnter: vi.fn(async () => () => undefined),
		listenDragLeave: vi.fn(async () => () => undefined),
	};
}

describe('app around the engine', () => {
	let runtime: AppRuntime | undefined;

	afterEach(() => {
		cleanup();
		runtime?.dispose();
		runtime = undefined;
	});

	async function renderWith(engine: FakeEngine): Promise<void> {
		const app = await createAppRuntime({ input: fakeInput(), engine });
		runtime = app;
		render(() => (
			<AppRuntimeProvider runtime={app}>
				<App />
			</AppRuntimeProvider>
		));
	}

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
		await renderWith(engine);
		await screen.findByTestId('left-column');

		runtime?.input.toggleSort();
		expect(await screen.findByText('The list is locked.')).toBeInTheDocument();
		await userEvent.click(screen.getByRole('button', { name: 'Dismiss' }));
		expect(screen.queryByText('The list is locked.')).toBeNull();
	});
});
