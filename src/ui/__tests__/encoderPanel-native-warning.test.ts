import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render } from '@testing-library/svelte';
import EncoderPanelIsland from '../encoderPanel/EncoderPanelIsland.svelte';

const context = vi.hoisted(() => ({
	listAvailableEncodersMock: vi.fn(),
}));

vi.mock('../../lib/tauri/client', () => ({
	tauriClient: {
		listAvailableEncoders: context.listAvailableEncodersMock,
	},
}));

describe('encoder panel native AAC warning', () => {
	beforeEach(() => {
		vi.resetModules();
		context.listAvailableEncodersMock.mockReset();
		localStorage.clear();
	});

	it('shows native AAC quality warning when auto resolves to native AAC', async () => {
		context.listAvailableEncodersMock.mockResolvedValue({
			fdkAvailable: false,
			aacAtAvailable: false,
			nativeAacAvailable: true,
		});

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
		context.listAvailableEncodersMock.mockResolvedValue({
			fdkAvailable: false,
			aacAtAvailable: true,
			nativeAacAvailable: true,
		});

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
		context.listAvailableEncodersMock.mockResolvedValue({
			fdkAvailable: true,
			aacAtAvailable: true,
			nativeAacAvailable: true,
		});

		const { initEncoderPanel } = await import('../encoderPanel');
		render(EncoderPanelIsland);
		initEncoderPanel();

		await vi.waitFor(() => {
			const hint = document.getElementById('encoder-availability-hint');
			expect(hint?.textContent).toBe('Auto will use FDK AAC.');
			const select = document.getElementById('adv-encoder') as HTMLSelectElement | null;
			expect(select?.options[0]?.textContent).toBe('Auto (FDK AAC)');
		});
	});

	it('restores the auto option label when a manual encoder is selected', async () => {
		context.listAvailableEncodersMock.mockResolvedValue({
			fdkAvailable: true,
			aacAtAvailable: true,
			nativeAacAvailable: true,
		});

		const { initEncoderPanel } = await import('../encoderPanel');
		render(EncoderPanelIsland);
		initEncoderPanel();

		await vi.waitFor(() => {
			const select = document.getElementById('adv-encoder') as HTMLSelectElement | null;
			expect(select?.options[0]?.textContent).toBe('Auto (FDK AAC)');
		});

		const select = document.getElementById('adv-encoder') as HTMLSelectElement;
		select.value = 'aac_at';
		select.dispatchEvent(new Event('change'));

		expect(select.options[0]?.textContent).toBe('Auto');
		const hint = document.getElementById('encoder-availability-hint');
		expect(hint?.textContent).toBe('Apple AAC available');
	});

	it('shows the native warning when Native AAC is manually selected', async () => {
		context.listAvailableEncodersMock.mockResolvedValue({
			fdkAvailable: true,
			aacAtAvailable: true,
			nativeAacAvailable: true,
		});

		const { initEncoderPanel } = await import('../encoderPanel');
		render(EncoderPanelIsland);
		initEncoderPanel();

		await vi.waitFor(() => {
			const select = document.getElementById('adv-encoder') as HTMLSelectElement | null;
			expect(select?.options[0]?.textContent).toBe('Auto (FDK AAC)');
		});

		const select = document.getElementById('adv-encoder') as HTMLSelectElement;
		select.value = 'native_aac';
		select.dispatchEvent(new Event('change'));

		const hint = document.getElementById('encoder-availability-hint');
		expect(hint?.textContent).toContain('Native AAC (FFmpeg) may sound degraded');
		expect(select.options[0]?.textContent).toBe('Auto');
	});
});
