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
		getSupportedAudioImportMetadata: vi.fn(async () => ({
			formats: [],
			extensions: [],
			formatsText: '',
			supportText: '',
		})),
		readAudioCoverThumbnail: vi.fn(async () => null),
		listenDragDrop: vi.fn(async () => () => undefined),
		listenDragEnter: vi.fn(async () => () => undefined),
		listenDragLeave: vi.fn(async () => () => undefined),
	};
}

describe('app before and around the engine', () => {
	let runtime: AppRuntime | undefined;

	afterEach(() => {
		cleanup();
		runtime?.dispose();
		runtime = undefined;
	});

	function renderWith(engine: FakeEngine): void {
		const app = createAppRuntime({ input: fakeInput(), engine });
		runtime = app;
		render(() => (
			<AppRuntimeProvider runtime={app}>
				<App />
			</AppRuntimeProvider>
		));
	}

	it('shows nothing the engine owns until its first snapshot arrives', async () => {
		const engine = createFakeEngine();
		const attach = engine.attach.bind(engine);
		let release!: () => void;
		const released = new Promise<void>((resolve) => {
			release = resolve;
		});
		engine.attach = async () => {
			await released;
			return attach();
		};
		renderWith(engine);

		expect(screen.queryByTestId('left-column')).toBeNull();
		release();
		expect(await screen.findByTestId('left-column')).toBeInTheDocument();
	});

	it('says so when the engine cannot be reached', async () => {
		const engine = createFakeEngine();
		engine.attach = async () => {
			throw new Error('engine unavailable');
		};
		renderWith(engine);

		expect(await screen.findByRole('alert')).toHaveTextContent('engine unavailable');
		expect(screen.queryByTestId('left-column')).toBeNull();
	});

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
		renderWith(engine);
		await screen.findByTestId('left-column');

		runtime?.input.toggleSort();
		expect(await screen.findByText('The list is locked.')).toBeInTheDocument();
		await userEvent.click(screen.getByRole('button', { name: 'Dismiss' }));
		expect(screen.queryByText('The list is locked.')).toBeNull();
	});
});
