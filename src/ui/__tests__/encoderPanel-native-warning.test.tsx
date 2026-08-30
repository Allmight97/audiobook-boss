import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { render } from '@solidjs/testing-library';
import type { AppRuntime } from '../../app/runtime';
import { createTestAppRuntime } from '../../app/runtime/harness';
import { AppRuntimeProvider } from '../../app/runtime/RuntimeProvider';
import { EncoderView } from '../encoderPanel/EncoderView';
import { resetEncoderPanelState } from '../encoderPanel/state';
import {
	encoderAvailabilityFixture,
	runtimeSettingsCapabilitiesFixture,
} from '../../test/fixtures/runtimeSettingsCapabilities';
import {
	runtimeSettingsCapabilitiesState,
	setRuntimeSettingsCapabilities,
} from '../runtimeSettingsCapabilities';

const context = vi.hoisted(() => ({
	getRuntimeSettingsCapabilitiesMock: vi.fn(),
}));

vi.mock('../../lib/tauri/client', () => ({
	tauriClient: {
		getRuntimeSettingsCapabilities: context.getRuntimeSettingsCapabilitiesMock,
		openFile: vi.fn(),
		updateAppSettings: vi.fn().mockResolvedValue(undefined),
	},
}));

const changeSelectValue = (select: HTMLSelectElement, value: string): void => {
	select.value = value;
	select.dispatchEvent(new Event('change', { bubbles: true }));
};

describe('encoder panel native AAC warning', () => {
	let runtime: AppRuntime | undefined;

	afterEach(() => {
		runtime?.dispose();
		runtime = undefined;
	});

	function renderEncoder() {
		runtime?.dispose();
		runtime = createTestAppRuntime();
		return render(() => (
			<AppRuntimeProvider runtime={runtime!}>
				<EncoderView />
			</AppRuntimeProvider>
		));
	}

	beforeEach(() => {
		context.getRuntimeSettingsCapabilitiesMock.mockReset();
		resetEncoderPanelState();
		setRuntimeSettingsCapabilities(null);
		runtimeSettingsCapabilitiesState.loading = false;
	});

	it('shows native AAC quality warning when auto resolves to native AAC', async () => {
		context.getRuntimeSettingsCapabilitiesMock.mockResolvedValue(
			runtimeSettingsCapabilitiesFixture({
				encoder: {
					availability: encoderAvailabilityFixture({
						fdkAvailable: false,
						aacAtAvailable: false,
						nativeAacAvailable: true,
					}),
				},
			}),
		);

		const { initializeEncoderPanelLogic } = await import('../encoderPanel/logic');
		renderEncoder();
		initializeEncoderPanelLogic();

		await vi.waitFor(() => {
			const hint = document.getElementById('encoder-availability-hint');
			expect(hint?.textContent).toContain('Auto will use Native AAC (FFmpeg).');
			expect(hint?.textContent).toContain('Native AAC (FFmpeg) may sound degraded');
			const select = document.getElementById('adv-encoder') as HTMLSelectElement | null;
			expect(select?.options[0]?.textContent).toBe('Auto (Native AAC (FFmpeg))');
		});
	});

	it('shows Apple resolution in the auto selector when non-native encoder is effective', async () => {
		context.getRuntimeSettingsCapabilitiesMock.mockResolvedValue(
			runtimeSettingsCapabilitiesFixture({
				encoder: {
					availability: encoderAvailabilityFixture({
						fdkAvailable: false,
						aacAtAvailable: true,
						nativeAacAvailable: true,
					}),
				},
			}),
		);

		const { initializeEncoderPanelLogic } = await import('../encoderPanel/logic');
		renderEncoder();
		initializeEncoderPanelLogic();

		await vi.waitFor(() => {
			const hint = document.getElementById('encoder-availability-hint');
			expect(hint?.textContent).toBe('Auto will use Apple AAC.');
			const select = document.getElementById('adv-encoder') as HTMLSelectElement | null;
			expect(select?.options[0]?.textContent).toBe('Auto (Apple AAC)');
		});
	});

	it('shows FDK resolution in the auto selector when FDK is available', async () => {
		context.getRuntimeSettingsCapabilitiesMock.mockResolvedValue(
			runtimeSettingsCapabilitiesFixture({
				encoder: {
					availability: encoderAvailabilityFixture({
						fdkAvailable: true,
						aacAtAvailable: true,
						nativeAacAvailable: true,
					}),
				},
			}),
		);

		const { initializeEncoderPanelLogic } = await import('../encoderPanel/logic');
		renderEncoder();
		initializeEncoderPanelLogic();

		await vi.waitFor(() => {
			const hint = document.getElementById('encoder-availability-hint');
			expect(hint?.textContent).toBe(
				'Using external FDK AAC via /opt/homebrew/bin/ffmpeg. Afterburner on.',
			);
			const select = document.getElementById('adv-encoder') as HTMLSelectElement | null;
			expect(select?.options[0]?.textContent).toBe('Auto (FDK AAC)');
		});
	});

	it('restores the auto option label when a manual encoder is selected', async () => {
		context.getRuntimeSettingsCapabilitiesMock.mockResolvedValue(
			runtimeSettingsCapabilitiesFixture({
				encoder: {
					availability: encoderAvailabilityFixture({
						fdkAvailable: true,
						aacAtAvailable: true,
						nativeAacAvailable: true,
					}),
				},
			}),
		);

		const { initializeEncoderPanelLogic } = await import('../encoderPanel/logic');
		renderEncoder();
		initializeEncoderPanelLogic();

		await vi.waitFor(() => {
			const select = document.getElementById('adv-encoder') as HTMLSelectElement | null;
			expect(select?.options[0]?.textContent).toBe('Auto (FDK AAC)');
		});

		const select = document.getElementById('adv-encoder') as HTMLSelectElement;
		changeSelectValue(select, 'aac_at');

		await vi.waitFor(() => {
			expect(select.options[0]?.textContent).toBe('Auto');
			const hint = document.getElementById('encoder-availability-hint');
			expect(hint?.textContent).toBe('Apple AAC available');
		});
	});

	it('shows the native warning when Native AAC is manually selected', async () => {
		context.getRuntimeSettingsCapabilitiesMock.mockResolvedValue(
			runtimeSettingsCapabilitiesFixture({
				encoder: {
					availability: encoderAvailabilityFixture({
						fdkAvailable: true,
						aacAtAvailable: true,
						nativeAacAvailable: true,
					}),
				},
			}),
		);

		const { initializeEncoderPanelLogic } = await import('../encoderPanel/logic');
		renderEncoder();
		initializeEncoderPanelLogic();

		await vi.waitFor(() => {
			const select = document.getElementById('adv-encoder') as HTMLSelectElement | null;
			expect(select?.options[0]?.textContent).toBe('Auto (FDK AAC)');
		});

		const select = document.getElementById('adv-encoder') as HTMLSelectElement;
		changeSelectValue(select, 'native_aac');

		await vi.waitFor(() => {
			const hint = document.getElementById('encoder-availability-hint');
			expect(hint?.textContent).toContain('Native AAC (FFmpeg) may sound degraded');
			expect(select.options[0]?.textContent).toBe('Auto');
		});
	});
});
