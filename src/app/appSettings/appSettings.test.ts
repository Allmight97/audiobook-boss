import { afterEach, describe, expect, it, vi } from 'vitest';
import type { AppSettings } from '../../types/appSettings';
import type { SettingsCapability } from '../../lib/tauri/capabilities/settings';
import { runtimeSettingsCapabilitiesFixture } from '../../test/fixtures/runtimeSettingsCapabilities';
import { createTestAppRuntime } from '../runtime/harness';
import type { AppRuntime } from '../runtime';

function settingsFixture(overrides: Partial<AppSettings> = {}): AppSettings {
	return {
		maxConcurrentJobs: { mode: 'auto' },
		encoderDefaults: {
			settings: {
				encoderType: 'auto',
				bitrateKbps: 64,
				bitrateMode: { mode: 'vbr', value: 3 },
				channels: 'auto',
				afterburner: true,
			},
			sampleRate: 'auto',
		},
		outputDefaults: {
			outputNaming: {
				preset: 'absDefault',
				includeYear: false,
			},
		},
		toolchain: {},
		startupBehavior: 'rememberLastState',
		defaultAcquisitionLane: 'audible',
		...overrides,
	};
}

function fakeSettings(overrides: Partial<SettingsCapability> = {}): SettingsCapability {
	return {
		getAppSettings: vi.fn(async () => settingsFixture()),
		updateAppSettings: vi.fn(async (patch) =>
			settingsFixture({
				maxConcurrentJobs: patch.maxConcurrentJobs ?? { mode: 'auto' },
				defaultAcquisitionLane:
					patch.defaultAcquisitionLane ?? settingsFixture().defaultAcquisitionLane,
			}),
		),
		resetAppSettings: vi.fn(async () => settingsFixture()),
		openFile: vi.fn(async () => null),
		getMaxConcurrentJobs: vi.fn(async () => 4),
		setMaxConcurrentJobs: vi.fn(async (value) => value ?? 4),
		getRuntimeSettingsCapabilities: vi.fn(async () => runtimeSettingsCapabilitiesFixture()),
		...overrides,
	};
}

describe('app settings concurrency', () => {
	let runtime: AppRuntime | undefined;

	afterEach(() => {
		runtime?.dispose();
		runtime = undefined;
	});

	it('hydrates auto selection and the effective backend count', async () => {
		const settings = fakeSettings();
		runtime = createTestAppRuntime({ settings });
		await runtime.settings.hydrateConcurrency();
		expect(runtime.settings.concurrency().effectiveLabel).toBe('Auto → 4');
		expect(runtime.settings.concurrency()).toMatchObject({
			selection: 'auto',
			effectiveLabel: 'Auto → 4',
			allowAuto: true,
			fixedOptions: [1, 2, 3, 4, 5, 6, 7, 8],
		});
		expect(settings.setMaxConcurrentJobs).toHaveBeenCalledWith(null);
	});

	it('persists a fixed selection and rolls back when the backend rejects it', async () => {
		const settings = fakeSettings({
			updateAppSettings: vi.fn(async () => {
				throw new Error('jobs active');
			}),
		});
		runtime = createTestAppRuntime({ settings });
		await runtime.settings.hydrateConcurrency();
		expect(runtime.settings.concurrency().effectiveLabel).toBe('Auto → 4');
		await runtime.settings.setConcurrencySelection('3');
		expect(runtime.settings.concurrency().selection).toBe('auto');
	});

	it('hydrates and persists defaultAcquisitionLane', async () => {
		const settings = fakeSettings({
			getAppSettings: vi.fn(async () => settingsFixture({ defaultAcquisitionLane: 'indexer' })),
		});
		runtime = createTestAppRuntime({ settings });
		await runtime.settings.hydrateAcquisitionPreferences();
		expect(runtime.settings.defaultAcquisitionLane()).toBe('indexer');

		await runtime.settings.setDefaultAcquisitionLane('audible');
		expect(settings.updateAppSettings).toHaveBeenCalledWith({ defaultAcquisitionLane: 'audible' });
		expect(runtime.settings.defaultAcquisitionLane()).toBe('audible');
	});
});
