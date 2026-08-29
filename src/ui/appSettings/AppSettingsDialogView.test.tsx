import { cleanup, fireEvent, render, screen } from '@solidjs/testing-library';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { AppSettings } from '../../types/appSettings';
import {
	productionSettingsDialogState,
	resetProductionSettingsDialog,
	setProductionSettingsDialogOpen,
} from '../../app/appSettings';
import { AppSettingsDialogView } from './AppSettingsDialogView';

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

describe('AppSettingsDialogView', () => {
	afterEach(() => {
		cleanup();
		resetProductionSettingsDialog();
	});

	beforeEach(() => {
		vi.clearAllMocks();
		context.getAppSettingsMock.mockResolvedValue(settingsFixture());
		context.resetAppSettingsMock.mockResolvedValue(settingsFixture());
		resetProductionSettingsDialog();
		productionSettingsDialogState.isOpen = true;
		productionSettingsDialogState.loading = false;
		productionSettingsDialogState.settings = settingsFixture();
	});

	it('requires a second activation before resetting all settings', async () => {
		render(() => <AppSettingsDialogView />);

		await fireEvent.click(screen.getByTestId('app-settings-reset'));
		expect(context.resetAppSettingsMock).not.toHaveBeenCalled();
		expect(screen.getByTestId('app-settings-reset-confirm-prompt')).toBeInTheDocument();

		await fireEvent.click(screen.getByTestId('app-settings-reset-confirm'));
		expect(context.resetAppSettingsMock).toHaveBeenCalledTimes(1);
	});

	it('returns to idle when the confirm step is cancelled', async () => {
		render(() => <AppSettingsDialogView />);

		await fireEvent.click(screen.getByTestId('app-settings-reset'));
		await fireEvent.click(screen.getByTestId('app-settings-reset-cancel'));

		expect(context.resetAppSettingsMock).not.toHaveBeenCalled();
		expect(screen.queryByTestId('app-settings-reset-confirm-prompt')).not.toBeInTheDocument();
		expect(screen.getByTestId('app-settings-reset')).toBeInTheDocument();
	});

	it('returns to idle when a click lands outside the reset row', async () => {
		render(() => <AppSettingsDialogView />);

		await fireEvent.click(screen.getByTestId('app-settings-reset'));
		await fireEvent.click(screen.getByRole('heading', { name: 'App Settings' }));

		expect(screen.queryByTestId('app-settings-reset-confirm-prompt')).not.toBeInTheDocument();
	});

	it('closes on Escape after opening post-mount, even with focus outside the dialog', async () => {
		resetProductionSettingsDialog();
		render(() => <AppSettingsDialogView />);
		expect(productionSettingsDialogState.isOpen).toBe(false);

		setProductionSettingsDialogOpen(true);
		await fireEvent.keyDown(document.body, { key: 'Escape' });

		expect(productionSettingsDialogState.isOpen).toBe(false);
	});
});
