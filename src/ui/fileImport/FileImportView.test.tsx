import { cleanup, fireEvent, render, screen } from '@solidjs/testing-library';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { createFakeEngine, defaultAppSettings } from '../../test/fixtures/fakeEngine';
import { AppRuntimeProvider, createAppRuntime, type AppRuntime } from '../../app/runtime';

import { FileImportView } from './FileImportView';

/** A runtime whose saved default lane is Indexer. */
async function runtimeDefaultingToIndexer(): Promise<AppRuntime> {
	const runtime = createAppRuntime({
		engine: createFakeEngine({ ...defaultAppSettings(), defaultAcquisitionLane: 'indexer' }),
	});
	await runtime.initialize();
	return runtime;
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
		runtime = await runtimeDefaultingToIndexer();
		render(() => (
			<AppRuntimeProvider runtime={runtime!}>
				<FileImportView />
			</AppRuntimeProvider>
		));

		await fireEvent.click(screen.getByRole('button', { name: 'Import' }));

		expect(runtime.remoteSource.view().isOpen).toBe(true);
		await vi.waitFor(() => expect(runtime!.remoteSource.view().providerId).toBe('indexer'));
	});

	it('opens Audible from the caret when default lane is Indexer', async () => {
		runtime = await runtimeDefaultingToIndexer();
		render(() => (
			<AppRuntimeProvider runtime={runtime!}>
				<FileImportView />
			</AppRuntimeProvider>
		));

		await fireEvent.click(screen.getByRole('button', { name: 'Import' }));
		await vi.waitFor(() => expect(runtime!.remoteSource.view().providerId).toBe('indexer'));
		runtime.remoteSource.close();
		expect(runtime.remoteSource.view().isOpen).toBe(false);
		await fireEvent.click(document.getElementById('import-split-caret') as Element);
		await fireEvent.click(screen.getByTestId('import-lane-audible'));

		expect(runtime.remoteSource.view().isOpen).toBe(true);
		await vi.waitFor(() => expect(runtime!.remoteSource.view().providerId).toBe('audible'));
		expect(document.getElementById('import-split-caret')).toHaveAttribute('aria-expanded', 'false');
	});
});
