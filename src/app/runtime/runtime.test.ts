import { afterEach, describe, expect, it, vi } from 'vitest';
import type { AcquisitionJob } from '../../types/remoteSource';
import { liveMetadataCapability } from '../../lib/tauri/capabilities/metadata';
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

	it('does not share view-local state across live runtimes', () => {
		const first = createAppRuntime();
		const second = createAppRuntime();
		dispose = () => {
			first.dispose();
			second.dispose();
		};

		void first.settings.openDialog();
		first.lookup.setTitleQuery('stale lookup');
		first.processing.pushTransientStatus('first runtime only');
		void first.remoteSource.open();
		first.remoteSource.editSearch({ titleFilter: 'first runtime only' });

		expect(first.settings.dialog().isOpen).toBe(true);
		expect(second.settings.dialog().isOpen).toBe(false);
		expect(first.lookup.view().titleQuery).toBe('stale lookup');
		expect(second.lookup.view().titleQuery).toBe('');
		expect(first.processing.status().statusText).toBe('first runtime only');
		expect(second.processing.status().statusText).toBe('Idle');
		expect(first.remoteSource.view().isOpen).toBe(true);
		expect(second.remoteSource.view().isOpen).toBe(false);
		expect(second.remoteSource.view().statusMessage).toBe('');

		first.dispose();
		first.processing.pushTransientStatus('after dispose');
		expect(second.settings.dialog().isOpen).toBe(false);
		expect(second.lookup.view().titleQuery).toBe('');
		expect(second.processing.status().statusText).toBe('Idle');
		expect(second.remoteSource.view().isOpen).toBe(false);

		const third = createAppRuntime();
		dispose = () => {
			second.dispose();
			third.dispose();
		};
		expect(third.settings.dialog().isOpen).toBe(false);
		expect(third.lookup.view().titleQuery).toBe('');
		expect(third.encoding.view().flavor).toBe('native_aac');
		expect(third.processing.status().statusText).toBe('Idle');
		expect(third.remoteSource.view().isOpen).toBe(false);
	});

	it('keeps lookup cover preview cancellation and cache isolated across runtimes', async () => {
		const firstLoad = createDeferred<number[]>();
		const first = createAppRuntime({
			metadata: {
				...liveMetadataCapability,
				loadCoverArtFromUrl: () => firstLoad.promise,
			},
		});
		const second = createAppRuntime({
			metadata: {
				...liveMetadataCapability,
				loadCoverArtFromUrl: async () => [0xff, 0xd8, 0xff],
			},
		});
		dispose = () => {
			first.dispose();
			second.dispose();
		};

		first.lookup.scheduleCoverPreviews(['https://covers.example/first.jpg']);
		second.lookup.scheduleCoverPreviews(['https://covers.example/second.jpg']);
		await vi.waitFor(() =>
			expect(second.lookup.coverPreview('https://covers.example/second.jpg').status).toBe('ready'),
		);

		first.dispose();
		firstLoad.resolve([0xff, 0xd8, 0xff]);
		await Promise.resolve();
		expect(first.lookup.coverPreview('https://covers.example/first.jpg').status).toBe('idle');
		expect(second.lookup.coverPreview('https://covers.example/second.jpg').status).toBe('ready');
	});

	it('a new runtime finds the session a disposed one left, without its view-local state', async () => {
		const engine = createFakeEngine();
		const first = createAppRuntime({ engine });
		engine.loadTitles([audioFile('/books/alpha.m4b')]);
		await first.initialize();
		first.input.setDragOver(true);
		expect(first.input.view().isDragOver).toBe(true);
		expect(first.input.view().fileCount).toBe(1);

		first.dispose();

		const second = createAppRuntime({ engine });
		dispose = () => second.dispose();
		await second.initialize();
		expect(engine.sessionIntents).not.toContainEqual({ kind: 'reset' });
		expect(second.input.view().isDragOver).toBe(false);
		expect(second.input.view().fileCount).toBe(1);
	});

	it('publishes nothing when the engine answers after disposal', async () => {
		const attached = createDeferred<void>();
		const engine = createFakeEngine();
		engine.change((state) => {
			state.output.directory = '/late';
		});
		const attach = engine.attach.bind(engine);
		engine.attach = async () => {
			await attached.promise;
			return attach();
		};
		const runtime = createAppRuntime({ engine });
		const startup = runtime.initialize();
		runtime.dispose();
		const outputAfterDispose = runtime.output.view();

		attached.resolve();
		await startup;

		expect(runtime.output.view()).toEqual(outputAfterDispose);
	});

	it('resets Remote Source owner state on dispose so a remount does not keep the dialog open', () => {
		const runtime = createAppRuntime();
		void runtime.remoteSource.open();
		expect(runtime.remoteSource.view().isOpen).toBe(true);
		runtime.dispose();
		expect(runtime.remoteSource.view().isOpen).toBe(false);
		expect(runtime.remoteSource.view().statusMessage).toBe('');
	});

	it('reattaches remote progress without publishing into a disposed runtime', async () => {
		const engine = createFakeEngine();
		engine.change((state) => {
			state.remote.acquisition = runningJob();
		});
		const runtime = createAppRuntime({ engine });
		await runtime.initialize();
		const before = runtime.remoteSource.view().activeJob;
		runtime.dispose();
		const other = createAppRuntime({ engine });
		dispose = () => other.dispose();
		await other.initialize();
		engine.change((state) => {
			state.remote.acquisition = {
				...runningJob(),
				progress: { ...runningJob().progress, percentage: 75 },
			};
		});
		expect(other.remoteSource.view().activeJob?.progress.percentage).toBe(75);
		expect(runtime.remoteSource.view().activeJob).toEqual(before);
		expect(runtime.remoteSource.view().isOpen).toBe(false);
	});
});
