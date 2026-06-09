import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { AppSettings } from '../../types/appSettings';
import { runtimeSettingsCapabilitiesFixture } from '../../test/fixtures/runtimeSettingsCapabilities';

const context = vi.hoisted(() => ({
	getAppSettingsMock: vi.fn(),
	updateAppSettingsMock: vi.fn(),
	applyEncodingDefaultsMock: vi.fn(),
	applyOutputDefaultsFromSettingsMock: vi.fn(),
	applyMaxConcurrentPreferenceMock: vi.fn(),
	loadRuntimeSettingsCapabilitiesMock: vi.fn(),
}));

vi.mock('../../lib/tauri/client', () => ({
	tauriClient: {
		getAppSettings: context.getAppSettingsMock,
		updateAppSettings: context.updateAppSettingsMock,
	},
}));

vi.mock('../encoderPanel', () => ({
	applyEncodingDefaults: context.applyEncodingDefaultsMock,
}));

vi.mock('../outputPanel', () => ({
	applyOutputDefaultsFromSettings: context.applyOutputDefaultsFromSettingsMock,
}));

vi.mock('../jobControls', () => ({
	applyMaxConcurrentPreference: context.applyMaxConcurrentPreferenceMock,
}));

vi.mock('../runtimeSettingsCapabilities.svelte', () => ({
	loadRuntimeSettingsCapabilities: context.loadRuntimeSettingsCapabilitiesMock,
}));

const settingsFixture = (): AppSettings => ({
	maxConcurrentJobs: { mode: 'fixed', value: 3 },
	encoderDefaults: {
		settings: {
			encoderType: 'native_aac',
			bitrateKbps: 64,
			bitrateMode: { mode: 'cbr' },
			channels: 'mono',
			afterburner: false,
			threads: { mode: 'auto' },
			twoloop: true,
		},
		sampleRate: { explicit: 44100 },
	},
	outputDefaults: {
		outputDirectory: '/books/out',
		outputNaming: {
			preset: 'customTemplate',
			includeYear: false,
			customTemplate: '{author}/{title}',
		},
	},
});

describe('app settings control plane', () => {
	beforeEach(() => {
		vi.clearAllMocks();
		context.updateAppSettingsMock.mockResolvedValue(settingsFixture());
		context.loadRuntimeSettingsCapabilitiesMock.mockResolvedValue(
			runtimeSettingsCapabilitiesFixture(),
		);
	});

	it('hydrates existing control owners from backend App Settings', async () => {
		const settings = settingsFixture();
		const capabilities = runtimeSettingsCapabilitiesFixture({
			maxConcurrentJobs: { fixedOptions: [1, 2, 3] },
		});
		context.getAppSettingsMock.mockResolvedValueOnce(settings);
		context.loadRuntimeSettingsCapabilitiesMock.mockResolvedValueOnce(capabilities);

		const { hydrateAppSettings } = await import('./hydration');
		await hydrateAppSettings();

		expect(context.loadRuntimeSettingsCapabilitiesMock).toHaveBeenCalledTimes(1);
		expect(context.applyEncodingDefaultsMock).toHaveBeenCalledWith(
			settings.encoderDefaults,
			capabilities.encoder,
		);
		expect(context.applyOutputDefaultsFromSettingsMock).toHaveBeenCalledWith(
			settings.outputDefaults,
		);
		expect(context.applyMaxConcurrentPreferenceMock).toHaveBeenCalledWith(
			settings.maxConcurrentJobs,
			capabilities.maxConcurrentJobs,
		);
	});

	it('continues hydrating other control owners when one owner fails', async () => {
		const settings = settingsFixture();
		context.getAppSettingsMock.mockResolvedValueOnce(settings);
		context.applyEncodingDefaultsMock.mockRejectedValueOnce(new Error('encoder failed'));

		const { hydrateAppSettings } = await import('./hydration');
		await hydrateAppSettings();

		expect(context.loadRuntimeSettingsCapabilitiesMock).toHaveBeenCalledTimes(1);
		expect(context.applyEncodingDefaultsMock).toHaveBeenCalledWith(
			settings.encoderDefaults,
			expect.any(Object),
		);
		expect(context.applyOutputDefaultsFromSettingsMock).toHaveBeenCalledWith(
			settings.outputDefaults,
		);
		expect(context.applyMaxConcurrentPreferenceMock).toHaveBeenCalledWith(
			settings.maxConcurrentJobs,
			expect.any(Object),
		);
	});

	it('delegates null capability slices when runtime capability loading fails', async () => {
		const settings = settingsFixture();
		context.getAppSettingsMock.mockResolvedValueOnce(settings);
		context.loadRuntimeSettingsCapabilitiesMock.mockResolvedValueOnce(null);

		const { hydrateAppSettings } = await import('./hydration');
		await hydrateAppSettings();

		expect(context.loadRuntimeSettingsCapabilitiesMock).toHaveBeenCalledTimes(1);
		expect(context.applyEncodingDefaultsMock).toHaveBeenCalledWith(settings.encoderDefaults, null);
		expect(context.applyMaxConcurrentPreferenceMock).toHaveBeenCalledWith(
			settings.maxConcurrentJobs,
			null,
		);
	});

	it('persists narrow accepted settings patches through tauriClient', async () => {
		const { persistConcurrencyPreference } = await import('./persistence');

		await persistConcurrencyPreference({ mode: 'fixed', value: 2 });

		expect(context.updateAppSettingsMock).toHaveBeenCalledWith({
			maxConcurrentJobs: { mode: 'fixed', value: 2 },
		});
	});
});
