import { render } from '@testing-library/svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import AppSettingsDialogIsland from '../appSettings/AppSettingsDialogIsland.svelte';
import { appSettingsDialogState } from '../appSettings/settingsDialog.svelte';
import { readFdkAfterburner } from '../encoderPanel';
import { resetEncoderPanelState } from '../encoderPanel/state.svelte';

const context = vi.hoisted(() => ({
	getAppSettingsMock: vi.fn(),
	updateAppSettingsMock: vi.fn(),
	resetAppSettingsMock: vi.fn(),
	openFileMock: vi.fn(),
	getRuntimeSettingsCapabilitiesMock: vi.fn(),
}));

vi.mock('../../lib/tauri/client', () => ({
	tauriClient: {
		getAppSettings: context.getAppSettingsMock,
		updateAppSettings: context.updateAppSettingsMock,
		resetAppSettings: context.resetAppSettingsMock,
		openFile: context.openFileMock,
		getRuntimeSettingsCapabilities: context.getRuntimeSettingsCapabilitiesMock,
	},
}));

describe('App Settings afterburner control', () => {
	beforeEach(() => {
		vi.clearAllMocks();
		context.updateAppSettingsMock.mockResolvedValue(undefined);
		resetEncoderPanelState();
		appSettingsDialogState.isOpen = true;
		appSettingsDialogState.loading = false;
	});

	it('owns the afterburner toggle and applies it to the encoder request truth', async () => {
		render(AppSettingsDialogIsland);

		const checkbox = document.getElementById('app-settings-afterburner') as HTMLInputElement | null;
		expect(checkbox).not.toBeNull();
		expect(checkbox?.checked).toBe(true);
		expect(readFdkAfterburner()).toBe(true);

		checkbox!.checked = false;
		checkbox!.dispatchEvent(new Event('change', { bubbles: true }));

		await vi.waitFor(() => {
			expect(readFdkAfterburner()).toBe(false);
		});
	});
});
