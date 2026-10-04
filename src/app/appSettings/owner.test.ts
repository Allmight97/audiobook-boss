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

	it('shows the engine concurrency', async () => {
		const app = open();
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

	it('keeps a dialog choice that could not be saved in effect and reports the save once', async () => {
		const app = open();
		await app.initialize();
		app.settings.openDialog();

		engine.settingsWriteError = { message: 'Read-only' };
		await app.settings.setStartupBehavior('pinnedDefaults');
		expect(app.settings.dialog().settings?.startupBehavior).toBe('pinnedDefaults');
		expect(app.settings.dialog().startupSaveState).not.toBe('error');
		expect(app.settings.durability()).toEqual({ state: 'error', message: 'Read-only' });

		engine.settingsWriteError = undefined;
		await app.settings.saveCurrentSettingsAsPinnedDefaults();
		expect(app.settings.dialog().settings?.pinnedDefaults).toBeDefined();
		expect(app.settings.durability().state).toBe('saved');

		await app.settings.resetAllAppSettings();
		expect(app.settings.dialog().saveState).toBe('saved');
		expect(app.settings.dialog().settings?.pinnedDefaults).toBeUndefined();
	});
});
