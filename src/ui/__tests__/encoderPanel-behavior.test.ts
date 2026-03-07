import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render } from '@testing-library/svelte';

import EncoderPanelIsland from '../encoderPanel/EncoderPanelIsland.svelte';
import { defaultEncoderSettings } from '../../types/audio';

const context = vi.hoisted(() => ({
	listAvailableEncodersMock: vi.fn(),
}));

vi.mock('../../lib/tauri/client', () => ({
	tauriClient: {
		listAvailableEncoders: context.listAvailableEncodersMock,
	},
}));

describe('encoder panel behavior controls', () => {
	beforeEach(() => {
		vi.resetModules();
		context.listAvailableEncodersMock.mockReset();
		localStorage.clear();
	});

	it('defaults to truthful default-on encoder behavior settings', () => {
		const defaults = defaultEncoderSettings();

		expect(defaults.afterburner).toBe(true);
		expect(defaults.twoloop).toBe(true);
	});

	it('shows afterburner for FDK and collapses the spacerless layout into behavior controls', async () => {
		context.listAvailableEncodersMock.mockResolvedValue({
			fdkAvailable: true,
			aacAtAvailable: true,
			nativeAacAvailable: true,
		});

		const { initEncoderPanel } = await import('../encoderPanel');
		render(EncoderPanelIsland);
		initEncoderPanel();

		await vi.waitFor(() => {
			const behaviorRow = document.getElementById('encoder-inline-option-row');
			const fdkOptions = document.getElementById('fdk-options');
			const afterburner = document.getElementById('adv-fdk-afterburner') as HTMLInputElement | null;
			expect(behaviorRow?.classList.contains('hidden')).toBe(true);
			expect(fdkOptions?.classList.contains('hidden')).toBe(true);
			expect(afterburner?.checked).toBe(true);
		});

		const select = document.getElementById('adv-encoder') as HTMLSelectElement;
		select.value = 'fdk_he_aac';
		select.dispatchEvent(new Event('change'));

		expect(document.getElementById('encoder-inline-option-row')?.classList.contains('hidden')).toBe(
			false,
		);
		expect(document.getElementById('fdk-options')?.classList.contains('hidden')).toBe(false);
	});

	it('shows twoloop only for manual Native AAC and hides all behavior controls for Apple AAC', async () => {
		context.listAvailableEncodersMock.mockResolvedValue({
			fdkAvailable: true,
			aacAtAvailable: true,
			nativeAacAvailable: true,
		});

		const { initEncoderPanel } = await import('../encoderPanel');
		render(EncoderPanelIsland);
		initEncoderPanel();

		await vi.waitFor(() => {
			expect(
				document.getElementById('encoder-inline-option-row')?.classList.contains('hidden'),
			).toBe(true);
		});

		const select = document.getElementById('adv-encoder') as HTMLSelectElement;
		select.value = 'native_aac';
		select.dispatchEvent(new Event('change'));

		expect(document.getElementById('native-options')?.classList.contains('hidden')).toBe(false);
		expect(
			(document.getElementById('adv-native-twoloop') as HTMLInputElement | null)?.checked,
		).toBe(true);

		select.value = 'aac_at';
		select.dispatchEvent(new Event('change'));

		expect(document.getElementById('encoder-inline-option-row')?.classList.contains('hidden')).toBe(
			true,
		);
		expect(document.getElementById('fdk-options')?.classList.contains('hidden')).toBe(true);
		expect(document.getElementById('native-options')?.classList.contains('hidden')).toBe(true);
	});

	it('respects persisted explicit opt-outs for afterburner and twoloop', async () => {
		localStorage.setItem(
			'abb.encoderPanel.v2',
			JSON.stringify({
				flavor: 'native_aac',
				fdkAfterburner: false,
				twoloop: false,
			}),
		);
		context.listAvailableEncodersMock.mockResolvedValue({
			fdkAvailable: true,
			aacAtAvailable: true,
			nativeAacAvailable: true,
		});

		const { initEncoderPanel } = await import('../encoderPanel');
		render(EncoderPanelIsland);
		initEncoderPanel();

		await vi.waitFor(() => {
			expect(
				document.getElementById('encoder-inline-option-row')?.classList.contains('hidden'),
			).toBe(true);
		});

		const select = document.getElementById('adv-encoder') as HTMLSelectElement;
		select.value = 'native_aac';
		select.dispatchEvent(new Event('change'));

		expect(
			(document.getElementById('adv-native-twoloop') as HTMLInputElement | null)?.checked,
		).toBe(false);

		select.value = 'auto';
		select.dispatchEvent(new Event('change'));
		expect(document.getElementById('encoder-inline-option-row')?.classList.contains('hidden')).toBe(
			true,
		);

		select.value = 'fdk_he_aac';
		select.dispatchEvent(new Event('change'));

		expect(
			(document.getElementById('adv-fdk-afterburner') as HTMLInputElement | null)?.checked,
		).toBe(false);
	});
});
