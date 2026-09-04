import { cleanup, fireEvent, render, screen } from '@solidjs/testing-library';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { AppSettings } from '../../types/appSettings';
import type { SettingsCapability } from '../../lib/tauri/capabilities/settings';
import { runtimeSettingsCapabilitiesFixture } from '../../test/fixtures/runtimeSettingsCapabilities';
import { AppRuntimeProvider } from '../../app/runtime/RuntimeProvider';
import { createTestAppRuntime } from '../../app/runtime/harness';
import type { AppRuntime } from '../../app/runtime';
import { tauriClient } from '../../lib/tauri/client';
import { AppSettingsDialogView } from './AppSettingsDialogView';

function settingsFixture(overrides: Partial<AppSettings> = {}): AppSettings {
	return {
		defaultAcquisitionLane: 'audible',
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
		...overrides,
	};
}

function fakeSettings(overrides: Partial<SettingsCapability> = {}): SettingsCapability {
	return {
		getAppSettings: vi.fn(async () => settingsFixture()),
		updateAppSettings: vi.fn(async (patch) =>
			settingsFixture({
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

describe('AppSettingsDialogView', () => {
	let runtime: AppRuntime | undefined;
	let settings: SettingsCapability;

	afterEach(() => {
		cleanup();
		runtime?.dispose();
		runtime = undefined;
	});

	async function renderOpenDialog(
		overrides: Partial<SettingsCapability> = {},
	): Promise<AppRuntime> {
		settings = fakeSettings(overrides);
		runtime = createTestAppRuntime({ settings });
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
		expect(settings.resetAppSettings).not.toHaveBeenCalled();
		expect(screen.getByTestId('app-settings-reset-confirm-prompt')).toBeInTheDocument();

		await fireEvent.click(screen.getByTestId('app-settings-reset-confirm'));
		expect(settings.resetAppSettings).toHaveBeenCalledTimes(1);
	});

	it('returns to idle when the confirm step is cancelled', async () => {
		await renderOpenDialog();

		await fireEvent.click(screen.getByTestId('app-settings-reset'));
		await fireEvent.click(screen.getByTestId('app-settings-reset-cancel'));

		expect(settings.resetAppSettings).not.toHaveBeenCalled();
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
		settings = fakeSettings();
		runtime = createTestAppRuntime({ settings });
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

	it('persists default acquisition lane when the Indexer radio is selected', async () => {
		await renderOpenDialog();

		await fireEvent.click(screen.getByTestId('app-settings-default-lane-indexer'));

		expect(settings.updateAppSettings).toHaveBeenCalledWith({
			defaultAcquisitionLane: 'indexer',
		});
		expect(runtime!.settings.defaultAcquisitionLane()).toBe('indexer');
	});

	it('lets the user enable both audiobook categories from the collapsed picker', async () => {
		await renderOpenDialog();

		expect(screen.getByTestId('app-settings-indexer-category')).toHaveTextContent(
			'Audiobooks (3030)',
		);
		await fireEvent.click(screen.getByTestId('app-settings-indexer-category'));
		const audio = screen.getByRole('checkbox', { name: 'Audio (3000)' }) as HTMLInputElement;
		audio.checked = true;
		audio.dispatchEvent(new Event('change', { bubbles: true }));

		expect(runtime!.remoteSource.indexerConnection().categoryIdsDraft).toEqual([3030, 3000]);
		expect(screen.getByTestId('app-settings-indexer-category')).toHaveTextContent(
			'Audiobooks (3030), Audio (3000)',
		);
	});

	it('keeps Indexer drafts when another Settings field changes', async () => {
		const getConnection = vi
			.spyOn(tauriClient, 'getRemoteSourceIndexerConnection')
			.mockResolvedValue({
				baseUrl: 'http://saved:9696',
				categoryIds: [3030],
				apiKeyConfigured: true,
			});
		const r = await renderOpenDialog();
		await vi.waitFor(() => expect(screen.getByLabelText('URL')).toHaveValue('http://saved:9696'));
		await fireEvent.input(screen.getByLabelText('URL'), { target: { value: 'http://draft:9696' } });
		await fireEvent.input(screen.getByLabelText('API key'), { target: { value: 'draft-secret' } });
		getConnection.mockClear();
		await fireEvent.input(screen.getByTestId('app-settings-ffmpeg-path'), {
			target: { value: '/tmp/ffmpeg' },
		});
		expect(getConnection).not.toHaveBeenCalled();
		expect(screen.getByLabelText('URL')).toHaveValue('http://draft:9696');
		expect(screen.getByLabelText('API key')).toHaveValue('draft-secret');
		expect(r.remoteSource.indexerConnection().apiKeyDraft).toBe('draft-secret');
	});

	it('dispatches Test for the current connection draft without saving or clearing the password', async () => {
		vi.spyOn(tauriClient, 'getRemoteSourceIndexerConnection').mockResolvedValue({
			baseUrl: 'http://saved:9696',
			categoryIds: [3030],
			apiKeyConfigured: true,
		});
		const r = await renderOpenDialog();
		await vi.waitFor(() => expect(screen.getByLabelText('URL')).toHaveValue('http://saved:9696'));
		const test = vi.spyOn(r.remoteSource, 'testIndexerConnection').mockResolvedValue();
		const save = vi.spyOn(r.remoteSource, 'saveIndexerConnectionSettings');
		const key = screen.getByLabelText('API key');
		expect(key).toHaveAttribute('type', 'password');
		await fireEvent.input(screen.getByLabelText('URL'), { target: { value: 'http://draft:9696' } });
		await fireEvent.input(key, { target: { value: 'draft-secret' } });
		await fireEvent.click(screen.getByRole('button', { name: 'Test' }));
		expect(test).toHaveBeenCalledOnce();
		expect(save).not.toHaveBeenCalled();
		expect(r.remoteSource.indexerConnection()).toMatchObject({
			baseUrlDraft: 'http://draft:9696',
			apiKeyDraft: 'draft-secret',
		});
		expect(key).toHaveValue('draft-secret');
	});
});
