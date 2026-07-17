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
	toolchain: {},
	startupBehavior: 'rememberLastState',
	density: 'comfortable',
	editSurface: 'rail',
	railWidth: 420,
});

const pinnedDefaultsFixture = (): NonNullable<AppSettings['pinnedDefaults']> => ({
	maxConcurrentJobs: { mode: 'fixed', value: 1 },
	encoderDefaults: {
		settings: {
			encoderType: 'native_aac',
			bitrateKbps: 96,
			bitrateMode: { mode: 'cbr' },
			channels: 'stereo',
			afterburner: false,
		},
		sampleRate: 'auto',
	},
	outputDefaults: {
		outputDirectory: '/books/pinned',
		outputNaming: {
			preset: 'absDefault',
			includeYear: true,
		},
	},
});

describe('app settings control plane', () => {
	beforeEach(() => {
		vi.clearAllMocks();
		delete document.documentElement.dataset.density;
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

	it('hydrates the rail width from settings and clamps it to the shell range', async () => {
		const settings = { ...settingsFixture(), railWidth: 512 };
		context.getAppSettingsMock.mockResolvedValueOnce(settings);

		const { hydrateAppSettings } = await import('./hydration');
		await hydrateAppSettings();

		const { readRailWidth } = await import('../appShell');
		expect(readRailWidth()).toBe(512);

		context.getAppSettingsMock.mockResolvedValueOnce({ ...settingsFixture(), railWidth: 10_000 });
		await hydrateAppSettings();
		expect(readRailWidth()).toBe(640);
	});

	it('hydrates compact density from the top-level preference, not pinned defaults', async () => {
		const settings = {
			...settingsFixture(),
			density: 'compact' as const,
			startupBehavior: 'pinnedDefaults' as const,
			pinnedDefaults: pinnedDefaultsFixture(),
		};
		context.getAppSettingsMock.mockResolvedValueOnce(settings);

		const { hydrateAppSettings } = await import('./hydration');
		await hydrateAppSettings();

		expect(document.documentElement.dataset.density).toBe('compact');
	});

	it('removes the density attribute for the comfortable preference', async () => {
		document.documentElement.dataset.density = 'compact';
		context.getAppSettingsMock.mockResolvedValueOnce(settingsFixture());

		const { hydrateAppSettings } = await import('./hydration');
		await hydrateAppSettings();

		expect(document.documentElement.hasAttribute('data-density')).toBe(false);
	});

	it('hydrates the edit-surface preference from the top-level value', async () => {
		const settings = {
			...settingsFixture(),
			editSurface: 'popover' as const,
		};
		context.getAppSettingsMock.mockResolvedValueOnce(settings);

		const { hydrateAppSettings } = await import('./hydration');
		await hydrateAppSettings();

		const { editSurfaceState } = await import('../metadataSurface');
		expect(editSurfaceState.preference).toBe('popover');
	});

	it('falls back to the rail edit surface when the settings field is absent', async () => {
		const settings = {
			...settingsFixture(),
			editSurface: undefined,
		} as unknown as AppSettings;
		context.getAppSettingsMock.mockResolvedValueOnce(settings);

		const { hydrateAppSettings } = await import('./hydration');
		await hydrateAppSettings();

		const { editSurfaceState } = await import('../metadataSurface');
		expect(editSurfaceState.preference).toBe('rail');
	});

	it('hydrates from the pinned defaults slot in pinnedDefaults startup mode', async () => {
		const pinned = pinnedDefaultsFixture();
		const settings = {
			...settingsFixture(),
			startupBehavior: 'pinnedDefaults' as const,
			pinnedDefaults: pinned,
		};
		context.getAppSettingsMock.mockResolvedValueOnce(settings);

		const { hydrateAppSettings } = await import('./hydration');
		await hydrateAppSettings();

		expect(context.applyEncodingDefaultsMock).toHaveBeenCalledWith(
			pinned.encoderDefaults,
			expect.anything(),
		);
		expect(context.applyOutputDefaultsFromSettingsMock).toHaveBeenCalledWith(pinned.outputDefaults);
		expect(context.applyMaxConcurrentPreferenceMock).toHaveBeenCalledWith(
			pinned.maxConcurrentJobs,
			expect.anything(),
		);
	});

	it('falls back to last-used values when pinned mode is set but nothing is pinned', async () => {
		const settings = {
			...settingsFixture(),
			startupBehavior: 'pinnedDefaults' as const,
		};
		context.getAppSettingsMock.mockResolvedValueOnce(settings);

		const { hydrateAppSettings } = await import('./hydration');
		await hydrateAppSettings();

		expect(context.applyEncodingDefaultsMock).toHaveBeenCalledWith(
			settings.encoderDefaults,
			expect.anything(),
		);
		expect(context.applyMaxConcurrentPreferenceMock).toHaveBeenCalledWith(
			settings.maxConcurrentJobs,
			expect.anything(),
		);
	});

	it('ignores a pinned slot while startup behavior is rememberLastState', async () => {
		const settings = {
			...settingsFixture(),
			pinnedDefaults: pinnedDefaultsFixture(),
		};
		context.getAppSettingsMock.mockResolvedValueOnce(settings);

		const { hydrateAppSettings } = await import('./hydration');
		await hydrateAppSettings();

		expect(context.applyEncodingDefaultsMock).toHaveBeenCalledWith(
			settings.encoderDefaults,
			expect.anything(),
		);
		expect(context.applyOutputDefaultsFromSettingsMock).toHaveBeenCalledWith(
			settings.outputDefaults,
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

	it('persists a density preference through the settings patch rail', async () => {
		const { persistAppSettingsPatch } = await import('./persistence');

		await persistAppSettingsPatch({ density: 'compact' });

		expect(context.updateAppSettingsMock).toHaveBeenCalledWith({ density: 'compact' });
	});
});
