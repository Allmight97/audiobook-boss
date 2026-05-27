import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { AppSettings } from '../../types/appSettings';

const context = vi.hoisted(() => ({
	getAppSettingsMock: vi.fn(),
	updateAppSettingsMock: vi.fn(),
	applyEncodingDefaultsMock: vi.fn(),
	applyOutputDefaultsFromSettingsMock: vi.fn(),
	applyMaxConcurrentPreferenceMock: vi.fn(),
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
		externalToolchain: { overridePath: '/opt/ffmpeg' },
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
	});

	it('hydrates existing control owners from backend App Settings', async () => {
		const settings = settingsFixture();
		context.getAppSettingsMock.mockResolvedValueOnce(settings);

		const { hydrateAppSettings } = await import('./hydration');
		await hydrateAppSettings();

		expect(context.applyEncodingDefaultsMock).toHaveBeenCalledWith(settings.encoderDefaults);
		expect(context.applyOutputDefaultsFromSettingsMock).toHaveBeenCalledWith(
			settings.outputDefaults,
		);
		expect(context.applyMaxConcurrentPreferenceMock).toHaveBeenCalledWith(
			settings.maxConcurrentJobs,
		);
	});

	it('continues hydrating other control owners when one owner fails', async () => {
		const settings = settingsFixture();
		context.getAppSettingsMock.mockResolvedValueOnce(settings);
		context.applyEncodingDefaultsMock.mockRejectedValueOnce(new Error('encoder failed'));

		const { hydrateAppSettings } = await import('./hydration');
		await hydrateAppSettings();

		expect(context.applyEncodingDefaultsMock).toHaveBeenCalledWith(settings.encoderDefaults);
		expect(context.applyOutputDefaultsFromSettingsMock).toHaveBeenCalledWith(
			settings.outputDefaults,
		);
		expect(context.applyMaxConcurrentPreferenceMock).toHaveBeenCalledWith(
			settings.maxConcurrentJobs,
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
