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

	it('shows native AAC quality warning when native is effective', async () => {
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
			expect(hint?.textContent).toContain('Native AAC may sound degraded');
		});
	});

	it('keeps standard availability hint when non-native encoder is effective', async () => {
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
			expect(hint?.textContent).toBe('Apple AAC available');
		});
	});
});
