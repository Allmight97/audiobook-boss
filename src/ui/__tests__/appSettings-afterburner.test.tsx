import userEvent from '@testing-library/user-event';
import { cleanup, render, screen } from '@solidjs/testing-library';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { AppRuntimeProvider, createAppRuntime, type AppRuntime } from '../../app/runtime';

import { AppSettingsDialogView } from '../appSettings/AppSettingsDialogView';

vi.mock('../../lib/tauri/client', async () => {
	const { runtimeSettingsCapabilitiesFixture } = await import(
		'../../test/fixtures/runtimeSettingsCapabilities'
	);
	return {
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
				keepAwakeWhileWorking: true,
			}),
			updateAppSettings: vi.fn().mockResolvedValue(undefined),
			resetAppSettings: vi.fn(),
			getAppSettingsRecovery: vi.fn().mockResolvedValue(null),
			recoverAppSettings: vi.fn(),
			openFile: vi.fn(),
			getRemoteSourceIndexerConnection: vi
				.fn()
				.mockResolvedValue({ baseUrl: null, categoryIds: [3030], apiKeyConfigured: false }),
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
		runtime = createAppRuntime();
		await runtime.settings.openDialog();
		runtime.encoding.select('intent', 'encode');
		render(() => (
			<AppRuntimeProvider runtime={runtime!}>
				<AppSettingsDialogView />
			</AppRuntimeProvider>
		));
	}

	it('toggles Afterburner from its keyboard-accessible info button', async () => {
		const user = userEvent.setup();
		await renderOpenDialog();
		await user.selectOptions(screen.getByLabelText('Encoder'), 'fdk_he_aac');
		const button = screen.getByRole('button', { name: 'FDK Afterburner' });
		expect(button).toHaveAttribute('aria-pressed', 'false');
		await user.hover(button);
		expect(screen.getByRole('tooltip')).toHaveTextContent('Afterburner off');
		await user.click(button);
		expect(button).toHaveAttribute('aria-pressed', 'true');
		expect(runtime!.encoding.view().afterburner).toBe(true);
		expect(screen.getByRole('tooltip')).toHaveTextContent('Click to disable.');
		await user.keyboard('{Escape}');
		expect(screen.queryByRole('tooltip')).not.toBeInTheDocument();
		expect(runtime!.settings.dialog().isOpen).toBe(true);
		await user.keyboard(' ');
		expect(button).toHaveAttribute('aria-pressed', 'false');
		expect(runtime!.encoding.view().afterburner).toBe(false);
	});

	it('retains a saved Afterburner choice across encoder switches', async () => {
		const user = userEvent.setup();
		await renderOpenDialog();
		await user.selectOptions(screen.getByLabelText('Encoder'), 'fdk_he_aac');
		runtime!.encoding.applyDefaults({
			...runtime!.encoding.readDefaults(),
			settings: { ...runtime!.encoding.readDefaults().settings, afterburner: true },
		});
		const button = screen.getByRole('button', { name: 'FDK Afterburner' });
		await vi.waitFor(() => expect(button).toHaveAttribute('aria-pressed', 'true'));
		await user.selectOptions(screen.getByLabelText('Encoder'), 'native_aac');
		expect(screen.queryByRole('button', { name: 'FDK Afterburner' })).not.toBeInTheDocument();
		await user.selectOptions(screen.getByLabelText('Encoder'), 'fdk_he_aac');
		expect(screen.getByRole('button', { name: 'FDK Afterburner' })).toHaveAttribute(
			'aria-pressed',
			'true',
		);
	});
});
