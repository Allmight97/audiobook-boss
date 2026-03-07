import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render } from '@testing-library/svelte';
import EncoderPanelIsland from '../encoderPanel/EncoderPanelIsland.svelte';
import { defaultEncoderSettings } from '../../types/audio';
import { resetEncoderPanelState } from '../encoderPanel/state.svelte';
import { outputPanelState } from '../outputPanel/state.svelte';

const context = vi.hoisted(() => ({
	listAvailableEncodersMock: vi.fn(),
	getCurrentFileListMock: vi.fn(),
}));

vi.mock('../../lib/tauri/client', () => ({
	tauriClient: {
		listAvailableEncoders: context.listAvailableEncodersMock,
	},
}));

vi.mock('../fileList', () => ({
	getCurrentFileList: context.getCurrentFileListMock,
}));

const changeSelectValue = (select: HTMLSelectElement, value: string): void => {
	select.value = value;
	select.dispatchEvent(new Event('change', { bubbles: true }));
};

describe('encoder panel behavior controls', () => {
	beforeEach(() => {
		context.listAvailableEncodersMock.mockReset();
		context.getCurrentFileListMock.mockReset();
		context.getCurrentFileListMock.mockReturnValue({
			files: [{ path: '/books/a.m4b', isValid: true }],
			totalDuration: 3600,
		});
		localStorage.clear();
		resetEncoderPanelState();
		outputPanelState.estimatedSizeText = '~ --- MB';
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
		changeSelectValue(select, 'fdk_he_aac');

		await vi.waitFor(() => {
			expect(
				document.getElementById('encoder-inline-option-row')?.classList.contains('hidden'),
			).toBe(false);
			expect(document.getElementById('fdk-options')?.classList.contains('hidden')).toBe(false);
		});
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
		changeSelectValue(select, 'native_aac');

		await vi.waitFor(() => {
			expect(document.getElementById('native-options')?.classList.contains('hidden')).toBe(false);
			expect(
				(document.getElementById('adv-native-twoloop') as HTMLInputElement | null)?.checked,
			).toBe(true);
		});

		changeSelectValue(select, 'aac_at');

		await vi.waitFor(() => {
			expect(
				document.getElementById('encoder-inline-option-row')?.classList.contains('hidden'),
			).toBe(true);
			expect(document.getElementById('fdk-options')?.classList.contains('hidden')).toBe(true);
			expect(document.getElementById('native-options')?.classList.contains('hidden')).toBe(true);
		});
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
		changeSelectValue(select, 'native_aac');

		await vi.waitFor(() => {
			expect(
				(document.getElementById('adv-native-twoloop') as HTMLInputElement | null)?.checked,
			).toBe(false);
		});

		changeSelectValue(select, 'auto');

		await vi.waitFor(() => {
			expect(
				document.getElementById('encoder-inline-option-row')?.classList.contains('hidden'),
			).toBe(true);
		});

		changeSelectValue(select, 'fdk_he_aac');

		await vi.waitFor(() => {
			expect(
				(document.getElementById('adv-fdk-afterburner') as HTMLInputElement | null)?.checked,
			).toBe(false);
		});
	});

	it('updates estimated size when encoder bitrate and channel choices change', async () => {
		context.listAvailableEncodersMock.mockResolvedValue({
			fdkAvailable: true,
			aacAtAvailable: true,
			nativeAacAvailable: true,
		});

		const { initEncoderPanel } = await import('../encoderPanel');
		render(EncoderPanelIsland);
		initEncoderPanel();

		const encoderSelect = document.getElementById('adv-encoder') as HTMLSelectElement;
		changeSelectValue(encoderSelect, 'native_aac');

		await vi.waitFor(() => {
			expect(document.getElementById('output-quality')?.classList.contains('hidden')).toBe(true);
		});

		const bitrateSelect = document.getElementById('output-bitrate') as HTMLSelectElement;
		changeSelectValue(bitrateSelect, '48');

		await vi.waitFor(() => {
			expect(outputPanelState.estimatedSizeText).not.toBe('~ --- MB');
		});
		const lowValue = Number.parseFloat(outputPanelState.estimatedSizeText.replace(/[^\d.]+/g, ''));

		const channelsSelect = document.getElementById('output-channels') as HTMLSelectElement;
		changeSelectValue(channelsSelect, 'stereo');

		await vi.waitFor(() => {
			const highValue = Number.parseFloat(
				outputPanelState.estimatedSizeText.replace(/[^\d.]+/g, ''),
			);
			expect(highValue).toBeGreaterThan(lowValue);
		});
	});
});
