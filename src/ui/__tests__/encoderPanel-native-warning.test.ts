import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render } from '@testing-library/svelte';
import EncoderPanelIsland from '../encoderPanel/EncoderPanelIsland.svelte';
import { resetEncoderPanelState } from '../encoderPanel/state.svelte';

const context = vi.hoisted(() => ({
	listAvailableEncodersMock: vi.fn(),
}));

vi.mock('../../lib/tauri/client', () => ({
	tauriClient: {
		listAvailableEncoders: context.listAvailableEncodersMock,
		refreshExternalToolchain: vi.fn(),
		open: vi.fn(),
	},
}));

const availabilityFixture = (overrides: {
	fdkAvailable: boolean;
	aacAtAvailable: boolean;
	nativeAacAvailable: boolean;
}) => ({
	...overrides,
	fdkSource: overrides.fdkAvailable ? 'detected' : 'none',
	autoEncoder: overrides.fdkAvailable
		? 'fdk_he_aac'
		: overrides.aacAtAvailable
			? 'aac_at'
			: 'native_aac',
	detectedToolchainPath: overrides.fdkAvailable ? '/opt/homebrew/bin/ffmpeg' : undefined,
	overrideToolchainPath: undefined,
	activeToolchainPath: overrides.fdkAvailable ? '/opt/homebrew/bin/ffmpeg' : undefined,
	overrideInvalid: false,
	overrideError: undefined,
	statusMessage: overrides.fdkAvailable
		? 'FDK AAC detected and ready.'
		: 'No external FFmpeg toolchain with libfdk_aac was detected.',
});

const changeSelectValue = (select: HTMLSelectElement, value: string): void => {
	select.value = value;
	select.dispatchEvent(new Event('change', { bubbles: true }));
};

describe('encoder panel native AAC warning', () => {
	beforeEach(() => {
		context.listAvailableEncodersMock.mockReset();
		resetEncoderPanelState();
	});

	it('shows native AAC quality warning when auto resolves to native AAC', async () => {
		context.listAvailableEncodersMock.mockResolvedValue(
			availabilityFixture({
				fdkAvailable: false,
				aacAtAvailable: false,
				nativeAacAvailable: true,
			}),
		);

		const { initEncoderPanel } = await import('../encoderPanel');
		render(EncoderPanelIsland);
		initEncoderPanel();

		await vi.waitFor(() => {
			const hint = document.getElementById('encoder-availability-hint');
			expect(hint?.textContent).toContain('Auto will use Native AAC (FFmpeg).');
			expect(hint?.textContent).toContain('Native AAC (FFmpeg) may sound degraded');
			const select = document.getElementById('adv-encoder') as HTMLSelectElement | null;
			expect(select?.options[0]?.textContent).toBe('Auto (Native AAC (FFmpeg))');
		});
	});

	it('shows Apple resolution in the auto selector when non-native encoder is effective', async () => {
		context.listAvailableEncodersMock.mockResolvedValue(
			availabilityFixture({
				fdkAvailable: false,
				aacAtAvailable: true,
				nativeAacAvailable: true,
			}),
		);

		const { initEncoderPanel } = await import('../encoderPanel');
		render(EncoderPanelIsland);
		initEncoderPanel();

		await vi.waitFor(() => {
			const hint = document.getElementById('encoder-availability-hint');
			expect(hint?.textContent).toBe('Auto will use Apple AAC.');
			const select = document.getElementById('adv-encoder') as HTMLSelectElement | null;
			expect(select?.options[0]?.textContent).toBe('Auto (Apple AAC)');
		});
	});

	it('shows FDK resolution in the auto selector when FDK is available', async () => {
		context.listAvailableEncodersMock.mockResolvedValue(
			availabilityFixture({
				fdkAvailable: true,
				aacAtAvailable: true,
				nativeAacAvailable: true,
			}),
		);

		const { initEncoderPanel } = await import('../encoderPanel');
		render(EncoderPanelIsland);
		initEncoderPanel();

		await vi.waitFor(() => {
			const hint = document.getElementById('encoder-availability-hint');
			expect(hint?.textContent).toContain(
				'Auto will use external FDK AAC at /opt/homebrew/bin/ffmpeg.',
			);
			const select = document.getElementById('adv-encoder') as HTMLSelectElement | null;
			expect(select?.options[0]?.textContent).toBe('Auto (FDK AAC)');
		});
	});

	it('restores the auto option label when a manual encoder is selected', async () => {
		context.listAvailableEncodersMock.mockResolvedValue(
			availabilityFixture({
				fdkAvailable: true,
				aacAtAvailable: true,
				nativeAacAvailable: true,
			}),
		);

		const { initEncoderPanel } = await import('../encoderPanel');
		render(EncoderPanelIsland);
		initEncoderPanel();

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
		context.listAvailableEncodersMock.mockResolvedValue(
			availabilityFixture({
				fdkAvailable: true,
				aacAtAvailable: true,
				nativeAacAvailable: true,
			}),
		);

		const { initEncoderPanel } = await import('../encoderPanel');
		render(EncoderPanelIsland);
		initEncoderPanel();

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
