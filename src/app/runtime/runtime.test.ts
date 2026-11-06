import { afterEach, describe, expect, it, vi } from 'vitest';
import type { AcquisitionJob } from '../../types/remoteSource';
import { audioFile, createFakeEngine } from '../../test/fixtures/fakeEngine';
import { createAppRuntime } from './index';

function createDeferred<T>() {
	let resolve!: (value: T) => void;
	const promise = new Promise<T>((resolvePromise) => {
		resolve = resolvePromise;
	});
	return { promise, resolve };
}

function runningJob(): AcquisitionJob {
	return {
		settled: false,
		terminal: false,
		jobId: 'remote-job-1',
		providerId: 'audible',
		status: 'acquiring',
		progress: {
			stage: 'download',
			percentage: 10,
			message: 'Downloading audiobook.',
			terminal: false,
			bytesDownloaded: null,
			bytesTotal: null,
			currentTitleId: null,
			currentItemIndex: null,
			totalItems: null,
		},
		materializedFiles: [],
		supplementalAssets: [],
		diagnostics: [],
	};
}

describe('app runtime', () => {
	let dispose: (() => void) | undefined;

	afterEach(() => {
		dispose?.();
		dispose = undefined;
	});

	it('does not share view-local state across live runtimes', async () => {
		const engine = createFakeEngine();
		engine.respond = (intent) => {
			if (intent.kind === 'lookupSetTitleQuery') {
				engine.change((state) => {
					state.lookup.titleQuery = intent.value;
				});
			}
			return intent.kind === 'submit'
				? {
						kind: 'rejected',
						error: { code: 'io_error', category: 'io', message: 'Disk full', detail: null },
					}
				: undefined;
		};
		const first = await createAppRuntime({ engine });
		const second = await createAppRuntime();
		dispose = () => {
			first.dispose();
			second.dispose();
		};

		first.settings.openDialog();
		first.lookup.setTitleQuery('stale lookup');
		await first.processing.start();
		void first.remoteSource.open({ lane: 'audible' });
		first.remoteSource.editSearch({ titleFilter: 'first runtime only' });

		expect(first.settings.dialog().isOpen).toBe(true);
		expect(second.settings.dialog().isOpen).toBe(false);
		expect(first.lookup.view().titleQuery).toBe('stale lookup');
		expect(second.lookup.view().titleQuery).toBe('');
		expect(first.processing.status().stepText).toBe('Error: Processing failed: Disk full');
		expect(second.processing.status().stepText).toBe('Current Step: Nothing running');
		expect(first.remoteSource.view().isOpen).toBe(true);
		expect(second.remoteSource.view().isOpen).toBe(false);
		expect(second.remoteSource.view().statusMessage).toBe('');

		first.dispose();
		expect(second.settings.dialog().isOpen).toBe(false);
		expect(second.lookup.view().titleQuery).toBe('');
		expect(second.processing.status().stepText).toBe('Current Step: Nothing running');
		expect(second.remoteSource.view().isOpen).toBe(false);

		const third = await createAppRuntime();
		dispose = () => {
			second.dispose();
			third.dispose();
		};
		expect(third.settings.dialog().isOpen).toBe(false);
		expect(third.lookup.view().titleQuery).toBe('');
		expect(third.encoding.view().effectiveFlavor).toBe('native_aac');
		expect(third.processing.status().stepText).toBe('Current Step: Nothing running');
		expect(third.remoteSource.view().isOpen).toBe(false);
	});

	it('a new runtime finds the session a disposed one left, without its view-local state', async () => {
		const engine = createFakeEngine();
		const first = await createAppRuntime({ engine });
		engine.loadTitles([audioFile('/books/alpha.m4b')]);
		first.input.setDragOver(true);
		expect(first.input.view().isDragOver).toBe(true);
		expect(first.input.view().fileCount).toBe(1);

		first.dispose();

		const second = await createAppRuntime({ engine });
		dispose = () => second.dispose();
		expect(engine.sessionIntents).not.toContainEqual({ kind: 'reset' });
		expect(second.input.view().isDragOver).toBe(false);
		expect(second.input.view().fileCount).toBe(1);
	});

	it('publishes nothing when the engine answers after disposal', async () => {
		const held = createDeferred<void>();
		const engine = createFakeEngine();
		engine.respond = async () => {
			await held.promise;
			engine.change((state) => {
				state.output.directory = '/late';
			});
			return undefined;
		};
		const runtime = await createAppRuntime({ engine });
		runtime.input.toggleSort();
		runtime.dispose();
		const outputAfterDispose = runtime.output.view();

		held.resolve();
		await vi.waitFor(() => expect(engine.sessionIntents).toHaveLength(1));
		await new Promise((resolve) => setTimeout(resolve));

		expect(runtime.output.view()).toEqual(outputAfterDispose);
	});

	it('reattaches remote progress without publishing into a disposed runtime', async () => {
		const engine = createFakeEngine();
		engine.change((state) => {
			state.remoteAcquisition.job = runningJob();
		});
		const runtime = await createAppRuntime({ engine });
		const before = runtime.remoteSource.acquisition();
		runtime.dispose();
		const other = await createAppRuntime({ engine });
		dispose = () => other.dispose();
		engine.change((state) => {
			state.remoteAcquisition.job = {
				...runningJob(),
				progress: { ...runningJob().progress, percentage: 75 },
			};
		});
		expect(other.remoteSource.acquisition()?.progress.percentage).toBe(75);
		expect(runtime.remoteSource.acquisition()).toEqual(before);
		expect(runtime.remoteSource.view().isOpen).toBe(false);
	});
});
