import { afterEach, describe, expect, it, vi } from 'vitest';
import {
	createFakeEngine,
	defaultAppSettings,
	type FakeEngine,
} from '../../test/fixtures/fakeEngine';
import { createAppRuntime, type AppRuntime } from '../runtime';

describe('settings owner', () => {
	let runtime: AppRuntime | undefined;
	let engine: FakeEngine;

	afterEach(() => {
		runtime?.dispose();
		runtime = undefined;
	});

	async function open(prepare: (engine: FakeEngine) => void = () => {}): Promise<AppRuntime> {
		engine = createFakeEngine();
		prepare(engine);
		runtime = await createAppRuntime({ engine });
		return runtime;
	}

	function unsaved(message: string) {
		return { code: 'io_error', category: 'io', message, detail: null } as const;
	}

	it('words the job count as Auto → N or Max N from the engine snapshot', async () => {
		const app = await open();

		expect(app.settings.concurrency()).toMatchObject({
			selection: 'auto',
			effective: 4,
			autoEffective: 4,
			effectiveLabel: 'Auto → 4',
		});
		engine.respondSettings = () => {
			const { concurrency } = engine.settings();
			engine.seedSettings({
				concurrency: { ...concurrency, preference: { mode: 'fixed', value: 2 }, effective: 2 },
			});
			return undefined;
		};
		await app.settings.setConcurrencySelection('2');
		expect(app.settings.concurrency()).toMatchObject({
			selection: '2',
			effective: 2,
			effectiveLabel: 'Max 2',
			errorMessage: '',
		});
	});

	it('shows an unsaved accepted default as an error until a retry saves it', async () => {
		const app = await open();

		engine.seedAppSettings(
			{ encoderDefaults: { ...engine.settings().settings.encoderDefaults, intent: 'preserve' } },
			unsaved('Disk full'),
		);

		await vi.waitFor(() =>
			expect(app.settings.durability()).toEqual({ state: 'error', message: 'Disk full' }),
		);
		engine.respondSettings = () => {
			engine.seedAppSettings({});
			return undefined;
		};
		await app.settings.retryPersistence();
		expect(app.settings.durability()).toEqual({ state: 'saved', message: '' });
		expect(engine.settings().settings?.encoderDefaults.intent).toBe('preserve');
	});

	it('shows the default import source as saving until the engine replies', async () => {
		const app = await open();
		let release: () => void = () => {};
		const replied = new Promise<void>((resolve) => {
			release = resolve;
		});
		engine.respondSettings = async () => {
			await replied;
			engine.seedAppSettings({ defaultAcquisitionLane: 'indexer' });
			return undefined;
		};

		const saving = app.settings.setDefaultAcquisitionLane('indexer');
		expect(app.settings.dialog().status.lane.state).toBe('saving');

		release();
		await saving;
		expect(app.settings.dialog().status.lane.state).toBe('saved');
		expect(app.settings.defaultAcquisitionLane()).toBe('indexer');
	});

	it('shows a refused default import source beside its choice', async () => {
		const app = await open();
		engine.settingsDispatch = async () => ({
			outcome: {
				kind: 'rejected',
				error: {
					code: 'invalid_input',
					category: 'validation',
					message: 'Indexer is not set up.',
					detail: null,
				},
			},
			snapshot: engine.settings(),
		});

		await app.settings.setDefaultAcquisitionLane('indexer');

		expect(app.settings.dialog().status.lane).toEqual({
			state: 'error',
			error: 'Indexer is not set up.',
		});
	});

	it('keeps a dialog choice that could not be saved in effect and reports the save once', async () => {
		const app = await open();
		app.settings.openDialog();

		engine.respondSettings = () => {
			engine.seedAppSettings({ startupBehavior: 'pinnedDefaults' }, unsaved('Read-only'));
			return undefined;
		};
		await app.settings.setStartupBehavior('pinnedDefaults');
		expect(app.settings.dialog().settings?.startupBehavior).toBe('pinnedDefaults');
		expect(app.settings.dialog().status.startup.state).not.toBe('error');
		expect(app.settings.durability()).toEqual({ state: 'error', message: 'Read-only' });

		engine.respondSettings = () => {
			const { settings } = engine.settings();
			engine.seedAppSettings({
				pinnedDefaults: {
					maxConcurrentJobs: settings.maxConcurrentJobs,
					encoderDefaults: settings.encoderDefaults,
					outputDefaults: settings.outputDefaults,
				},
			});
			return undefined;
		};
		await app.settings.saveCurrentSettingsAsPinnedDefaults();
		expect(app.settings.dialog().settings?.pinnedDefaults).toBeDefined();
		expect(app.settings.durability().state).toBe('saved');

		engine.respondSettings = () => {
			engine.seedSettings({ settings: defaultAppSettings() });
			return undefined;
		};
		await app.settings.resetAllAppSettings();
		expect(app.settings.dialog().status.reset.state).toBe('saved');
		expect(app.settings.dialog().settings?.pinnedDefaults).toBeNull();
	});
});
