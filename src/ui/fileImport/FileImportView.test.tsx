import { cleanup, fireEvent, render, screen } from '@solidjs/testing-library';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { createFakeEngine, defaultAppSettings } from '../../test/fixtures/fakeEngine';
import { AppRuntimeProvider, createAppRuntime, type AppRuntime } from '../../app/runtime';

import { FileImportView } from './FileImportView';

async function runtimeDefaultingToIndexer() {
	const engine = createFakeEngine({ ...defaultAppSettings(), defaultAcquisitionLane: 'indexer' });
	const runtime = await createAppRuntime({ engine });
	return { runtime, engine };
}

describe('FileImportView import split button', () => {
	let runtime: AppRuntime | undefined;

	afterEach(() => {
		cleanup();
		runtime?.dispose();
		runtime = undefined;
		document.body.innerHTML = '';
	});

	it('opens the default acquisition lane from settings on main click', async () => {
		const opened = await runtimeDefaultingToIndexer();
		runtime = opened.runtime;
		const engine = opened.engine;
		render(() => (
			<AppRuntimeProvider runtime={runtime!}>
				<FileImportView />
			</AppRuntimeProvider>
		));

		fireEvent.click(screen.getByRole('button', { name: 'Import' }));

		expect(runtime.remoteSource.view().isOpen).toBe(true);
		await vi.waitFor(() =>
			expect(engine.sessionIntents).toContainEqual({
				kind: 'remote',
				intent: { kind: 'selectLane', lane: 'indexer' },
			}),
		);
	});

	it('opens Audible from the caret when default lane is Indexer', async () => {
		const opened = await runtimeDefaultingToIndexer();
		runtime = opened.runtime;
		const engine = opened.engine;
		render(() => (
			<AppRuntimeProvider runtime={runtime!}>
				<FileImportView />
			</AppRuntimeProvider>
		));

		fireEvent.click(screen.getByRole('button', { name: 'Import' }));
		await vi.waitFor(() =>
			expect(engine.sessionIntents).toContainEqual({
				kind: 'remote',
				intent: { kind: 'selectLane', lane: 'indexer' },
			}),
		);
		runtime.remoteSource.close();
		expect(runtime.remoteSource.view().isOpen).toBe(false);
		fireEvent.click(document.getElementById('import-split-caret') as Element);
		fireEvent.click(screen.getByTestId('import-lane-audible'));

		expect(runtime.remoteSource.view().isOpen).toBe(true);
		await vi.waitFor(() =>
			expect(engine.sessionIntents).toContainEqual({
				kind: 'remote',
				intent: { kind: 'selectLane', lane: 'audible' },
			}),
		);
		expect(document.getElementById('import-split-caret')).toHaveAttribute('aria-expanded', 'false');
	});

	it('says drag and drop is not working when its listener cannot start', async () => {
		runtime = await createAppRuntime({
			engine: createFakeEngine(),
			input: {
				openFiles: vi.fn(async () => []),
				openDirectory: vi.fn(async () => null),
				listenDragEnter: vi.fn(async () => () => undefined),
				listenDragLeave: vi.fn(async () => () => undefined),
				listenDragDrop: vi.fn(async () => {
					throw new Error('no window');
				}),
			},
		});
		render(() => (
			<AppRuntimeProvider runtime={runtime!}>
				<FileImportView />
			</AppRuntimeProvider>
		));

		expect(await screen.findByText(/Drag and drop is not working/)).toBeInTheDocument();
	});
});
