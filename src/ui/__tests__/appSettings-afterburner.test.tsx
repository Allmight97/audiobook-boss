import { cleanup, render } from '@solidjs/testing-library';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { AppRuntimeProvider } from '../../app/runtime/RuntimeProvider';
import { createTestAppRuntime } from '../../app/runtime/harness';
import type { AppRuntime } from '../../app/runtime';
import { AppSettingsDialogView } from '../appSettings/AppSettingsDialogView';

vi.mock('../../lib/tauri/client', async () => {
	const { runtimeSettingsCapabilitiesFixture } = await import(
		'../../test/fixtures/runtimeSettingsCapabilities'
	);
	return {
		tauriClient: {
			getAppSettings: vi.fn().mockResolvedValue({
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
			getRuntimeSettingsCapabilities: vi
				.fn()
				.mockResolvedValue(runtimeSettingsCapabilitiesFixture()),
		},
	};
});

describe('App Settings afterburner control', () => {
	let runtime: AppRuntime | undefined;

	afterEach(() => {
		cleanup();
		runtime?.dispose();
		runtime = undefined;
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
		expect(runtime!.encoding.view().afterburner).toBe(true);

		checkbox!.checked = false;
		checkbox!.dispatchEvent(new Event('change', { bubbles: true }));

		await vi.waitFor(() => {
			expect(runtime!.encoding.view().afterburner).toBe(false);
		});
	});

	it('reflects encoder hydration after the dialog has already mounted', async () => {
		await renderOpenDialog();

		const checkbox = document.getElementById('app-settings-afterburner') as HTMLInputElement | null;
		expect(checkbox?.checked).toBe(true);

		runtime!.encoding.applyDefaults({
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
