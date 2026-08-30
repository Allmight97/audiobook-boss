import { cleanup, render } from '@solidjs/testing-library';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
	productionSettingsDialogState,
	resetProductionSettingsDialog,
} from '../../app/appSettings';
import { AppSettingsDialogView } from '../appSettings/AppSettingsDialogView';
import { readFdkAfterburner } from '../encoderPanel';
import { applyEncoderDefaults, resetEncoderPanelState } from '../encoderPanel/state';

vi.mock('../../lib/tauri/client', () => ({
	tauriClient: {
		getAppSettings: vi.fn(),
		updateAppSettings: vi.fn().mockResolvedValue(undefined),
		resetAppSettings: vi.fn(),
		openFile: vi.fn(),
		getRuntimeSettingsCapabilities: vi.fn(),
	},
}));

describe('App Settings afterburner control', () => {
	afterEach(() => {
		cleanup();
		resetProductionSettingsDialog();
	});

	beforeEach(() => {
		resetEncoderPanelState();
		resetProductionSettingsDialog();
		productionSettingsDialogState.isOpen = true;
		productionSettingsDialogState.loading = false;
	});

	it('owns the afterburner toggle and applies it to the encoder request truth', async () => {
		render(() => <AppSettingsDialogView />);

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

	it('reflects encoder hydration after the dialog has already mounted', async () => {
		render(() => <AppSettingsDialogView />);

		const checkbox = document.getElementById('app-settings-afterburner') as HTMLInputElement | null;
		expect(checkbox?.checked).toBe(true);

		applyEncoderDefaults({
			settings: {
				encoderType: 'auto',
				bitrateKbps: 64,
				bitrateMode: { mode: 'vbr', value: 3 },
				channels: 'auto',
				afterburner: false,
			},
			sampleRate: 'auto',
		});

		await vi.waitFor(() => {
			expect(checkbox?.checked).toBe(false);
		});
	});
});
