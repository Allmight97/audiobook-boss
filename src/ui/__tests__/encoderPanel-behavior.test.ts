import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render } from '@testing-library/svelte';
import EncoderPanelIsland from '../encoderPanel/EncoderPanelIsland.svelte';
import { defaultEncoderSettings } from '../../types/audio';
import { encoderPanelState, resetEncoderPanelState } from '../encoderPanel/state.svelte';
import { readEncodingRequestConfig } from '../encoderPanel';
import { outputPanelState } from '../outputPanel/state.svelte';
import {
	encoderAvailabilityFixture,
	runtimeSettingsCapabilitiesFixture,
} from '../../test/fixtures/runtimeSettingsCapabilities';
import { runtimeSettingsCapabilitiesState } from '../runtimeSettingsCapabilities.svelte';

const context = vi.hoisted(() => ({
	getRuntimeSettingsCapabilitiesMock: vi.fn(),
	getCurrentFileListMock: vi.fn(),
}));

vi.mock('../../lib/tauri/client', () => ({
	tauriClient: {
		getRuntimeSettingsCapabilities: context.getRuntimeSettingsCapabilitiesMock,
		openFile: vi.fn(),
		updateAppSettings: vi.fn().mockResolvedValue(undefined),
	},
}));

vi.mock('../fileList/state.svelte', () => ({
	getCurrentFileList: context.getCurrentFileListMock,
	isOrderLocked: vi.fn(() => false),
	onOrderLockChange: vi.fn(() => () => undefined),
}));

const changeSelectValue = (select: HTMLSelectElement, value: string): void => {
	select.value = value;
	select.dispatchEvent(new Event('change', { bubbles: true }));
};

const waitForEncoderOptions = async (): Promise<void> => {
	await vi.waitFor(() => {
		const select = document.getElementById('adv-encoder') as HTMLSelectElement | null;
		expect(select?.options.length).toBeGreaterThan(1);
	});
};

describe('encoder panel behavior controls', () => {
	beforeEach(() => {
		context.getRuntimeSettingsCapabilitiesMock.mockReset();
		context.getCurrentFileListMock.mockReset();
		context.getCurrentFileListMock.mockReturnValue({
			files: [{ path: '/books/a.m4b', isValid: true }],
			totalDuration: 3600,
		});
		resetEncoderPanelState();
		runtimeSettingsCapabilitiesState.capabilities = null;
		runtimeSettingsCapabilitiesState.loadError = null;
		runtimeSettingsCapabilitiesState.loading = false;
		outputPanelState.estimatedSizeText = '~ --- MB';
	});

	it('defaults to truthful default-on encoder behavior settings', () => {
		const defaults = defaultEncoderSettings();

		expect(defaults.afterburner).toBe(true);
		expect(defaults.twoloop).toBe(true);
	});

	it('shows afterburner for FDK and collapses the spacerless layout into behavior controls', async () => {
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
		render(EncoderPanelIsland);
		initializeEncoderPanelLogic();
		await waitForEncoderOptions();

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

	it('keeps afterburner enabled in encoding config when auto resolves to FDK', async () => {
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
		render(EncoderPanelIsland);
		initializeEncoderPanelLogic();
		await waitForEncoderOptions();

		await vi.waitFor(() => {
			const config = readEncodingRequestConfig();
			expect(config.encoderSettings.encoderType).toBe('auto');
			expect(config.encoderSettings.afterburner).toBe(true);
		});
	});

	it('renders encoder option ranges from runtime capabilities', async () => {
		context.getRuntimeSettingsCapabilitiesMock.mockResolvedValue(
			runtimeSettingsCapabilitiesFixture({
				encoder: {
					availability: encoderAvailabilityFixture({
						fdkAvailable: true,
						aacAtAvailable: true,
						nativeAacAvailable: true,
					}),
					bitrateKbpsOptions: [64, 96],
					explicitSampleRates: [44100],
					channelOptions: ['auto', 'mono'],
				},
			}),
		);

		const { initializeEncoderPanelLogic } = await import('../encoderPanel/logic');
		render(EncoderPanelIsland);
		initializeEncoderPanelLogic();
		await waitForEncoderOptions();

		await vi.waitFor(() => {
			const bitrateValues = Array.from(
				(document.getElementById('output-bitrate') as HTMLSelectElement).options,
			).map((option) => option.value);
			const sampleRateValues = Array.from(
				(document.getElementById('output-samplerate') as HTMLSelectElement).options,
			).map((option) => option.value);
			const channelValues = Array.from(
				(document.getElementById('output-channels') as HTMLSelectElement).options,
			).map((option) => option.value);

			expect(bitrateValues).toEqual(['64', '96']);
			expect(sampleRateValues).toEqual(['auto', '44100']);
			expect(channelValues).toEqual(['auto', 'mono']);
		});
	});

	it('shows twoloop only for manual Native AAC and hides all behavior controls for Apple AAC', async () => {
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
		render(EncoderPanelIsland);
		initializeEncoderPanelLogic();
		await waitForEncoderOptions();

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

	it('retains session opt-outs for afterburner and twoloop across re-init', async () => {
		encoderPanelState.flavor = 'native_aac';
		encoderPanelState.fdkAfterburner = false;
		encoderPanelState.nativeTwoloop = false;
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
		render(EncoderPanelIsland);
		initializeEncoderPanelLogic();

		await vi.waitFor(() => {
			expect(
				document.getElementById('encoder-inline-option-row')?.classList.contains('hidden'),
			).toBe(true);
			expect(encoderPanelState.fdkAfterburner).toBe(false);
			expect(encoderPanelState.nativeTwoloop).toBe(false);
		});

		initializeEncoderPanelLogic();
		await vi.waitFor(() => {
			expect(encoderPanelState.fdkAfterburner).toBe(false);
			expect(encoderPanelState.nativeTwoloop).toBe(false);
		});
	});

	it('updates estimated size when encoder bitrate and channel choices change', async () => {
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
		render(EncoderPanelIsland);
		initializeEncoderPanelLogic();
		await waitForEncoderOptions();

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

	it('uses the capability VBR range without underestimating higher quality levels', async () => {
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
		render(EncoderPanelIsland);
		initializeEncoderPanelLogic();
		await waitForEncoderOptions();

		const qualitySelect = document.getElementById('output-quality') as HTMLSelectElement;
		changeSelectValue(qualitySelect, '5');

		await vi.waitFor(() => {
			expect(document.getElementById('estimated-bitrate')?.textContent).toBe('Est: ~96 kbps');
		});
	});

	it('keeps the current session toolchain override path on init', async () => {
		encoderPanelState.externalToolchainOverridePath = '/custom/ffmpeg';
		context.getRuntimeSettingsCapabilitiesMock.mockResolvedValue(
			runtimeSettingsCapabilitiesFixture({
				encoder: {
					availability: {
						...encoderAvailabilityFixture({
							fdkAvailable: true,
							aacAtAvailable: true,
							nativeAacAvailable: true,
						}),
						fdkSource: 'override',
						overrideToolchainPath: '/custom/ffmpeg',
						activeToolchainPath: '/custom/ffmpeg',
						statusMessage: 'FDK AAC is using the saved override path.',
					},
				},
			}),
		);

		const { initializeEncoderPanelLogic } = await import('../encoderPanel/logic');
		render(EncoderPanelIsland);
		initializeEncoderPanelLogic();

		await vi.waitFor(() => {
			expect(document.getElementById('external-toolchain-path-display')?.textContent).toContain(
				'/custom/ffmpeg',
			);
			expect(readEncodingRequestConfig().toolchainSettings.overridePath).toBe('/custom/ffmpeg');
		});
	});

	it('shows a compact missing-FDK state with the override input when detection fails', async () => {
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
		render(EncoderPanelIsland);
		initializeEncoderPanelLogic();

		await vi.waitFor(() => {
			expect(document.getElementById('external-toolchain-status')?.textContent).toContain(
				'FDK not found',
			);
			expect(document.getElementById('external-toolchain-path')).toBeTruthy();
			expect(document.getElementById('external-toolchain-mode')).toBeFalsy();
		});
	});

	it('refreshes FDK availability without requiring restart', async () => {
		context.getRuntimeSettingsCapabilitiesMock
			.mockResolvedValueOnce(
				runtimeSettingsCapabilitiesFixture({
					encoder: {
						availability: encoderAvailabilityFixture({
							fdkAvailable: false,
							aacAtAvailable: true,
							nativeAacAvailable: true,
						}),
					},
				}),
			)
			.mockResolvedValueOnce(
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
		render(EncoderPanelIsland);
		initializeEncoderPanelLogic();

		await vi.waitFor(() => {
			const select = document.getElementById('adv-encoder') as HTMLSelectElement | null;
			expect(select?.options[0]?.textContent).toBe('Auto (Apple AAC)');
		});

		(document.getElementById('toolchain-refresh') as HTMLButtonElement).click();

		await vi.waitFor(() => {
			const select = document.getElementById('adv-encoder') as HTMLSelectElement | null;
			expect(select?.options[0]?.textContent).toBe('Auto (FDK AAC)');
			expect(document.getElementById('external-toolchain-path-display')?.textContent).toContain(
				'/opt/homebrew/bin/ffmpeg',
			);
		});
	});

	it('keeps the override input visible when the current session path is invalid', async () => {
		encoderPanelState.externalToolchainOverridePath = '/broken/ffmpeg';
		context.getRuntimeSettingsCapabilitiesMock.mockResolvedValue(
			runtimeSettingsCapabilitiesFixture({
				encoder: {
					availability: {
						...encoderAvailabilityFixture({
							fdkAvailable: true,
							aacAtAvailable: true,
							nativeAacAvailable: true,
						}),
						overrideToolchainPath: '/broken/ffmpeg',
						overrideInvalid: true,
						overrideError: "FFmpeg executable not found at 'ffmpeg'.",
						statusMessage: 'Saved override path is invalid. Auto-detected FDK AAC is active.',
					},
				},
			}),
		);

		const { initializeEncoderPanelLogic } = await import('../encoderPanel/logic');
		render(EncoderPanelIsland);
		initializeEncoderPanelLogic();

		await vi.waitFor(() => {
			expect(document.getElementById('external-toolchain-status')?.textContent).toContain(
				'Saved override path is invalid',
			);
			expect(document.getElementById('external-toolchain-path')).toBeTruthy();
			expect(document.getElementById('external-toolchain-error')?.textContent).toContain(
				'not found',
			);
			const select = document.getElementById('adv-encoder') as HTMLSelectElement | null;
			expect(select?.options[0]?.textContent).toBe('Auto (FDK AAC)');
		});
	});

	it('normalizes a session-selected unavailable Apple AAC flavor back to auto', async () => {
		encoderPanelState.flavor = 'aac_at';
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
		render(EncoderPanelIsland);
		initializeEncoderPanelLogic();

		await vi.waitFor(() => {
			const select = document.getElementById('adv-encoder') as HTMLSelectElement | null;
			expect(select?.value).toBe('auto');
			expect(select?.options[0]?.textContent).toBe('Auto (Native AAC (FFmpeg))');
			expect(document.getElementById('encoder-availability-hint')?.textContent).toContain(
				'Auto will use Native AAC (FFmpeg).',
			);
			expect(readEncodingRequestConfig().encoderSettings.encoderType).toBe('auto');
		});
	});

	it('normalizes a session-selected unavailable native AAC flavor back to auto', async () => {
		encoderPanelState.flavor = 'native_aac';
		context.getRuntimeSettingsCapabilitiesMock.mockResolvedValue(
			runtimeSettingsCapabilitiesFixture({
				encoder: {
					availability: encoderAvailabilityFixture({
						fdkAvailable: false,
						aacAtAvailable: true,
						nativeAacAvailable: false,
					}),
				},
			}),
		);

		const { initializeEncoderPanelLogic } = await import('../encoderPanel/logic');
		render(EncoderPanelIsland);
		initializeEncoderPanelLogic();

		await vi.waitFor(() => {
			const select = document.getElementById('adv-encoder') as HTMLSelectElement | null;
			expect(select?.value).toBe('auto');
			expect(select?.options[0]?.textContent).toBe('Auto (Apple AAC)');
			expect(document.getElementById('encoder-availability-hint')?.textContent).toBe(
				'Auto will use Apple AAC.',
			);
			expect(readEncodingRequestConfig().encoderSettings.encoderType).toBe('auto');
		});
	});
});
