import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { AppSettings } from '../../types/appSettings';
import { runtimeSettingsCapabilitiesFixture } from '../../test/fixtures/runtimeSettingsCapabilities';

const context = vi.hoisted(() => ({
	getAppSettingsMock: vi.fn(),
	updateAppSettingsMock: vi.fn(),
	resetAppSettingsMock: vi.fn(),
	openFileMock: vi.fn(),
	refreshRuntimeSettingsCapabilitiesMock: vi.fn(),
	hydrateAppSettingsMock: vi.fn(),
}));

vi.mock('../../lib/tauri/client', () => ({
	tauriClient: {
		getAppSettings: context.getAppSettingsMock,
		updateAppSettings: context.updateAppSettingsMock,
		resetAppSettings: context.resetAppSettingsMock,
		openFile: context.openFileMock,
	},
}));

vi.mock('../runtimeSettingsCapabilities.svelte', () => ({
	refreshRuntimeSettingsCapabilities: context.refreshRuntimeSettingsCapabilitiesMock,
}));

vi.mock('./hydration', () => ({
	hydrateAppSettings: context.hydrateAppSettingsMock,
}));

const settingsFixture = (externalFfmpegPath?: string): AppSettings => ({
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
	toolchain: externalFfmpegPath ? { externalFfmpegPath } : {},
	startupBehavior: 'rememberLastState',
	density: 'comfortable',
	editSurface: 'rail',
	railWidth: 420,
});

async function loadDialogModule() {
	return import('./settingsDialog.svelte');
}

describe('app settings dialog', () => {
	beforeEach(() => {
		vi.clearAllMocks();
		vi.resetModules();
		context.getAppSettingsMock.mockResolvedValue(settingsFixture());
		context.updateAppSettingsMock.mockResolvedValue(settingsFixture('/opt/user/ffmpeg'));
		context.resetAppSettingsMock.mockResolvedValue(settingsFixture());
		context.hydrateAppSettingsMock.mockResolvedValue(undefined);
		context.refreshRuntimeSettingsCapabilitiesMock.mockResolvedValue(
			runtimeSettingsCapabilitiesFixture(),
		);
	});

	it('opens with persisted settings and refreshed toolchain status', async () => {
		context.getAppSettingsMock.mockResolvedValueOnce(settingsFixture('/opt/user/ffmpeg'));
		const dialog = await loadDialogModule();

		await dialog.openAppSettingsDialog();

		expect(dialog.appSettingsDialogState.isOpen).toBe(true);
		expect(dialog.appSettingsDialogState.ffmpegPathDraft).toBe('/opt/user/ffmpeg');
		expect(dialog.appSettingsDialogState.encoderAvailability?.statusMessage).toBe(
			'FDK AAC detected and ready.',
		);
		expect(context.refreshRuntimeSettingsCapabilitiesMock).toHaveBeenCalled();
	});

	it('saves the ffmpeg path as a toolchain patch and re-probes capabilities', async () => {
		const dialog = await loadDialogModule();
		await dialog.openAppSettingsDialog();
		dialog.appSettingsDialogState.ffmpegPathDraft = '  /opt/user/ffmpeg  ';

		await dialog.saveToolchainPreference();

		expect(context.updateAppSettingsMock).toHaveBeenCalledWith({
			toolchain: { externalFfmpegPath: '/opt/user/ffmpeg' },
		});
		expect(dialog.appSettingsDialogState.saveState).toBe('saved');
		expect(dialog.appSettingsDialogState.ffmpegPathDraft).toBe('/opt/user/ffmpeg');
		// One refresh on open, one after save: the save verdict must re-probe.
		expect(context.refreshRuntimeSettingsCapabilitiesMock).toHaveBeenCalledTimes(2);
	});

	it('clears the path by sending an unset toolchain preference', async () => {
		context.updateAppSettingsMock.mockResolvedValue(settingsFixture());
		const dialog = await loadDialogModule();
		await dialog.openAppSettingsDialog();
		dialog.appSettingsDialogState.ffmpegPathDraft = '   ';

		await dialog.saveToolchainPreference();

		expect(context.updateAppSettingsMock).toHaveBeenCalledWith({
			toolchain: { externalFfmpegPath: undefined },
		});
		expect(dialog.appSettingsDialogState.ffmpegPathDraft).toBe('');
	});

	it('surfaces persistence failures without closing the dialog', async () => {
		context.updateAppSettingsMock.mockRejectedValue(new Error('disk full'));
		const dialog = await loadDialogModule();
		await dialog.openAppSettingsDialog();
		dialog.appSettingsDialogState.ffmpegPathDraft = '/opt/user/ffmpeg';

		await dialog.saveToolchainPreference();

		expect(dialog.appSettingsDialogState.saveState).toBe('error');
		expect(dialog.appSettingsDialogState.saveError).toContain('disk full');
		expect(dialog.appSettingsDialogState.isOpen).toBe(true);
	});

	it('reset restores defaults and re-hydrates the owning controls', async () => {
		const dialog = await loadDialogModule();
		await dialog.openAppSettingsDialog();

		await dialog.resetAllAppSettings();

		expect(context.resetAppSettingsMock).toHaveBeenCalled();
		expect(context.hydrateAppSettingsMock).toHaveBeenCalled();
		expect(dialog.appSettingsDialogState.settings).toEqual(settingsFixture());
	});

	it('browse fills the draft only when a file is chosen', async () => {
		const dialog = await loadDialogModule();
		await dialog.openAppSettingsDialog();

		context.openFileMock.mockResolvedValueOnce(null);
		await dialog.browseForFfmpegBinary();
		expect(dialog.appSettingsDialogState.ffmpegPathDraft).toBe('');

		context.openFileMock.mockResolvedValueOnce('/picked/ffmpeg');
		await dialog.browseForFfmpegBinary();
		expect(dialog.appSettingsDialogState.ffmpegPathDraft).toBe('/picked/ffmpeg');
	});

	it('pins the current last-used settings values as pinned defaults', async () => {
		const current = settingsFixture();
		current.maxConcurrentJobs = { mode: 'fixed', value: 2 };
		current.outputDefaults.outputDirectory = '/books/out';
		const pinned = {
			maxConcurrentJobs: current.maxConcurrentJobs,
			encoderDefaults: current.encoderDefaults,
			outputDefaults: current.outputDefaults,
		};
		const dialog = await loadDialogModule();
		await dialog.openAppSettingsDialog();
		context.getAppSettingsMock.mockResolvedValueOnce(current);
		context.updateAppSettingsMock.mockResolvedValueOnce({
			...current,
			pinnedDefaults: pinned,
		});

		await dialog.saveCurrentSettingsAsPinnedDefaults();

		// Capture is a pure settings copy: current top-level values → pinned slot.
		expect(context.updateAppSettingsMock).toHaveBeenCalledWith({ pinnedDefaults: pinned });
		expect(dialog.appSettingsDialogState.startupSaveState).toBe('saved');
		expect(dialog.appSettingsDialogState.settings?.pinnedDefaults).toEqual(pinned);
	});

	it('persists the startup behavior toggle', async () => {
		const dialog = await loadDialogModule();
		await dialog.openAppSettingsDialog();
		context.updateAppSettingsMock.mockResolvedValueOnce({
			...settingsFixture(),
			startupBehavior: 'pinnedDefaults',
		});

		await dialog.setStartupBehavior('pinnedDefaults');

		expect(context.updateAppSettingsMock).toHaveBeenCalledWith({
			startupBehavior: 'pinnedDefaults',
		});
		expect(dialog.appSettingsDialogState.settings?.startupBehavior).toBe('pinnedDefaults');
	});

	it('surfaces pin failures without closing the dialog', async () => {
		const dialog = await loadDialogModule();
		await dialog.openAppSettingsDialog();
		context.updateAppSettingsMock.mockRejectedValueOnce(new Error('disk full'));

		await dialog.saveCurrentSettingsAsPinnedDefaults();

		expect(dialog.appSettingsDialogState.startupSaveState).toBe('error');
		expect(dialog.appSettingsDialogState.startupSaveError).toContain('disk full');
		expect(dialog.appSettingsDialogState.isOpen).toBe(true);
	});
});
