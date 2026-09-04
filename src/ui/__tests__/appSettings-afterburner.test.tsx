import { cleanup, render } from '@solidjs/testing-library';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { AppRuntimeProvider } from '../../app/runtime/RuntimeProvider';
import { createTestAppRuntime } from '../../app/runtime/harness';
import type { AppRuntime } from '../../app/runtime';
import { AppSettingsDialogView } from '../appSettings/AppSettingsDialogView';
import { readFdkAfterburner } from '../encoderPanel';
import { applyEncoderDefaults, resetEncoderPanelState } from '../encoderPanel/state';

vi.mock('../../lib/tauri/client', () => ({
	tauriClient: {
		getAppSettings: vi.fn().mockResolvedValue({
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
		}),
		updateAppSettings: vi.fn().mockResolvedValue(undefined),
		resetAppSettings: vi.fn(),
		openFile: vi.fn(),
		getRuntimeSettingsCapabilities: vi.fn().mockResolvedValue({
			encoder: { availability: null },
			maxConcurrentJobs: { allowAuto: true, fixedOptions: [1, 2, 4] },
		}),
		getRemoteSourceIndexerConnection: vi.fn().mockResolvedValue({
			baseUrl: null,
			categoryId: 3030,
			apiKeyConfigured: false,
		}),
		updateRemoteSourceIndexerConnection: vi.fn().mockResolvedValue({
			baseUrl: null,
			categoryId: 3030,
			apiKeyConfigured: false,
		}),
		testRemoteSourceIndexerConnection: vi.fn().mockResolvedValue({
			ok: true,
			message: 'Indexer connection test succeeded.',
		}),
	},
}));

describe('App Settings afterburner control', () => {
	let runtime: AppRuntime | undefined;

	afterEach(() => {
		cleanup();
		runtime?.dispose();
		runtime = undefined;
		resetEncoderPanelState();
	});

	beforeEach(() => {
		resetEncoderPanelState();
	});

	async function renderOpenDialog(): Promise<void> {
		runtime = createTestAppRuntime();
		await runtime.settings.openDialog();
		render(() => (
			<AppRuntimeProvider runtime={runtime!}>
				<AppSettingsDialogView />
			</AppRuntimeProvider>
		));
	}

	it('owns the afterburner toggle and applies it to the encoder request truth', async () => {
		await renderOpenDialog();

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
		await renderOpenDialog();

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
