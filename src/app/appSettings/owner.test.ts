import { afterEach, describe, expect, it, vi } from 'vitest';
import { createFakeEngine, type FakeEngine } from '../../test/fixtures/fakeEngine';
import { createAppRuntime, type AppRuntime } from '../runtime';

// Which settings are in effect, and whether they are saved, is the engine's
// to decide. These tests cover what this owner adds: how it shows the
// engine's answer, and the progress it keeps for each dialog control.

describe('settings owner', () => {
	let runtime: AppRuntime | undefined;
	let engine: FakeEngine;

	afterEach(() => {
		runtime?.dispose();
		runtime = undefined;
	});

	function open(prepare: (engine: FakeEngine) => void = () => {}): AppRuntime {
		engine = createFakeEngine();
		prepare(engine);
		runtime = createAppRuntime({ engine });
		return runtime;
	}

	it('shows the engine concurrency and nothing before the engine has answered', async () => {
		const app = open();
		expect(app.settings.concurrency()).toMatchObject({
			selection: 'auto',
			effective: null,
			autoEffective: null,
			fixedOptions: [],
			effectiveLabel: '',
		});

		await app.initialize();

		expect(app.settings.concurrency()).toMatchObject({
			selection: 'auto',
			effective: 4,
			autoEffective: 4,
			effectiveLabel: 'Auto → 4',
			allowAuto: true,
		});
		await app.settings.setConcurrencySelection('2');
		expect(app.settings.concurrency()).toMatchObject({
			selection: '2',
			effective: 2,
			effectiveLabel: 'Max 2',
			errorMessage: '',
		});
	});

	it('shows an unsaved accepted default as an error until a retry saves it', async () => {
		const app = open((engine) => {
			engine.settingsWriteError = { message: 'Disk full' };
		});
		await app.initialize();

		// The engine records a default the session chose, and the write fails.
		engine.recordSettings({
			encoderDefaults: { ...engine.settings().settings!.encoderDefaults, intent: 'preserve' },
		});

		await vi.waitFor(() =>
			expect(app.settings.durability()).toEqual({ state: 'error', message: 'Disk full' }),
		);
		engine.settingsWriteError = undefined;
		await app.settings.retryPersistence();
		expect(app.settings.durability()).toEqual({ state: 'saved', message: '' });
		expect(engine.settings().settings?.encoderDefaults.intent).toBe('preserve');
	});

	it('shows unreadable saved settings as the dialog error', async () => {
		const app = open((engine) => engine.breakSettings());

		await app.settings.openDialog();
		expect(app.settings.dialog()).toMatchObject({
			isOpen: true,
			loading: false,
			settings: null,
			saveState: 'error',
			saveError: 'App settings file could not be read by this version.',
		});
	});

	it('tracks each dialog control on its own through a reset', async () => {
		const app = open();
		await app.initialize();
		await app.settings.openDialog();

		engine.settingsWriteError = { message: 'Read-only' };
		await app.settings.setStartupBehavior('pinnedDefaults');
		expect(app.settings.dialog()).toMatchObject({
			startupSaveState: 'error',
			startupSaveError: 'Read-only',
			powerSaveState: 'idle',
			saveState: 'idle',
		});

		engine.settingsWriteError = undefined;
		await app.settings.saveCurrentSettingsAsPinnedDefaults();
		expect(app.settings.dialog().startupSaveState).toBe('saved');
		expect(app.settings.dialog().settings?.pinnedDefaults).toBeDefined();

		await app.settings.resetAllAppSettings();
		expect(app.settings.dialog().saveState).toBe('saved');
		expect(app.settings.dialog().settings?.pinnedDefaults).toBeUndefined();
	});
});
