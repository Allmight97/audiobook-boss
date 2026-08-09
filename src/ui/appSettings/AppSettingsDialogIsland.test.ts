import { fireEvent, render, screen } from '@testing-library/svelte';
import { tick } from 'svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { AppSettings } from '../../types/appSettings';
import AppSettingsDialogIsland from './AppSettingsDialogIsland.svelte';
import { appSettingsDialogState } from './settingsDialog.svelte';

const context = vi.hoisted(() => ({
	getAppSettingsMock: vi.fn(),
	updateAppSettingsMock: vi.fn(),
	resetAppSettingsMock: vi.fn(),
	openFileMock: vi.fn(),
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
	refreshRuntimeSettingsCapabilities: vi.fn().mockResolvedValue(null),
}));

function settingsFixture(): AppSettings {
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
	};
}

describe('AppSettingsDialogIsland', () => {
	beforeEach(() => {
		vi.clearAllMocks();
		context.getAppSettingsMock.mockResolvedValue(settingsFixture());
		context.resetAppSettingsMock.mockResolvedValue(settingsFixture());
		appSettingsDialogState.isOpen = true;
		appSettingsDialogState.loading = false;
		appSettingsDialogState.settings = settingsFixture();
	});

	it('requires a second activation before resetting all settings', async () => {
		render(AppSettingsDialogIsland);

		await fireEvent.click(screen.getByTestId('app-settings-reset'));
		expect(context.resetAppSettingsMock).not.toHaveBeenCalled();
		expect(screen.getByTestId('app-settings-reset-confirm-prompt')).toBeInTheDocument();

		await fireEvent.click(screen.getByTestId('app-settings-reset-confirm'));
		expect(context.resetAppSettingsMock).toHaveBeenCalledTimes(1);
	});

	it('returns to idle when the confirm step is cancelled', async () => {
		render(AppSettingsDialogIsland);

		await fireEvent.click(screen.getByTestId('app-settings-reset'));
		await fireEvent.click(screen.getByTestId('app-settings-reset-cancel'));

		expect(context.resetAppSettingsMock).not.toHaveBeenCalled();
		expect(screen.queryByTestId('app-settings-reset-confirm-prompt')).not.toBeInTheDocument();
		expect(screen.getByTestId('app-settings-reset')).toBeInTheDocument();
	});

	it('returns to idle when a click lands outside the reset row', async () => {
		render(AppSettingsDialogIsland);

		await fireEvent.click(screen.getByTestId('app-settings-reset'));
		await fireEvent.click(
			screen.getByTestId('app-settings-close').closest('.app-modal-header') as Element,
		);

		expect(screen.queryByTestId('app-settings-reset-confirm-prompt')).not.toBeInTheDocument();
	});

	it('closes on Escape after opening post-mount, even with focus outside the dialog', async () => {
		// Real flow: the island mounts closed and opens later. The regression
		// this pins: Escape must work even when focus never made it inside the
		// dialog (initial focus can be refused mid-visibility-transition in
		// WebKit), so the keydown is dispatched from document.body on purpose.
		appSettingsDialogState.isOpen = false;
		render(AppSettingsDialogIsland);
		expect(appSettingsDialogState.isOpen).toBe(false);

		appSettingsDialogState.isOpen = true;
		await tick();

		await fireEvent.keyDown(document.body, { key: 'Escape' });

		expect(appSettingsDialogState.isOpen).toBe(false);
	});
});
