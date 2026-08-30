import { cleanup, fireEvent, render, screen } from '@solidjs/testing-library';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { AppSettings } from '../../types/appSettings';
import { AppRuntimeProvider } from '../../app/runtime/RuntimeProvider';
import { createTestAppRuntime } from '../../app/runtime/harness';
import type { AppRuntime } from '../../app/runtime';
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
	let runtime: AppRuntime | undefined;

	afterEach(() => {
		cleanup();
		runtime?.dispose();
		runtime = undefined;
	});

	beforeEach(() => {
		vi.clearAllMocks();
		context.getAppSettingsMock.mockResolvedValue(settingsFixture());
		context.resetAppSettingsMock.mockResolvedValue(settingsFixture());
		context.getRuntimeSettingsCapabilitiesMock.mockResolvedValue({
			encoder: { availability: null },
			maxConcurrentJobs: { allowAuto: true, fixedOptions: [1, 2, 4] },
		});
	});

	async function renderOpenDialog(): Promise<AppRuntime> {
		runtime = createTestAppRuntime();
		await runtime.settings.openDialog();
		render(() => (
			<AppRuntimeProvider runtime={runtime!}>
				<AppSettingsDialogView />
			</AppRuntimeProvider>
		));
		return runtime;
	}

	it('requires a second activation before resetting all settings', async () => {
		await renderOpenDialog();

		await fireEvent.click(screen.getByTestId('app-settings-reset'));
		expect(context.resetAppSettingsMock).not.toHaveBeenCalled();
		expect(screen.getByTestId('app-settings-reset-confirm-prompt')).toBeInTheDocument();

		await fireEvent.click(screen.getByTestId('app-settings-reset-confirm'));
		expect(context.resetAppSettingsMock).toHaveBeenCalledTimes(1);
	});

	it('returns to idle when the confirm step is cancelled', async () => {
		await renderOpenDialog();

		await fireEvent.click(screen.getByTestId('app-settings-reset'));
		await fireEvent.click(screen.getByTestId('app-settings-reset-cancel'));

		expect(context.resetAppSettingsMock).not.toHaveBeenCalled();
		expect(screen.queryByTestId('app-settings-reset-confirm-prompt')).not.toBeInTheDocument();
		expect(screen.getByTestId('app-settings-reset')).toBeInTheDocument();
	});

	it('returns to idle when a click lands outside the reset row', async () => {
		await renderOpenDialog();

		await fireEvent.click(screen.getByTestId('app-settings-reset'));
		await fireEvent.click(screen.getByRole('heading', { name: 'App Settings' }));

		expect(screen.queryByTestId('app-settings-reset-confirm-prompt')).not.toBeInTheDocument();
	});

	it('closes on Escape after opening post-mount, even with focus outside the dialog', async () => {
		runtime = createTestAppRuntime();
		render(() => (
			<AppRuntimeProvider runtime={runtime!}>
				<AppSettingsDialogView />
			</AppRuntimeProvider>
		));
		expect(runtime.settings.dialog().isOpen).toBe(false);

		await runtime.settings.openDialog();
		await fireEvent.keyDown(document.body, { key: 'Escape' });

		expect(runtime.settings.dialog().isOpen).toBe(false);
	});
});
