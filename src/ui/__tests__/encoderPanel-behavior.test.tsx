import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { render } from '@solidjs/testing-library';
import type { AppRuntime } from '../../app/runtime';
import { createTestAppRuntime } from '../../app/runtime/harness';
import { AppRuntimeProvider } from '../../app/runtime/RuntimeProvider';
import { EncoderView } from '../encoderPanel/EncoderView';
import { encoderPanelState, resetEncoderPanelState } from '../encoderPanel/state';
import { readEncodingRequestConfig } from '../encoderPanel';
import { renderAutoResolutionHints } from '../encoderPanel/autoResolutionHints';
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

const waitForEncoderOptions = async (): Promise<void> => {
	await vi.waitFor(() => {
		const select = document.getElementById('adv-encoder') as HTMLSelectElement | null;
		expect(select?.options.length).toBeGreaterThan(1);
	});
};

describe('encoder panel behavior controls', () => {
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

	it('renders no afterburner control; the App Settings dialog owns it', async () => {
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
		await waitForEncoderOptions();

		expect(document.getElementById('fdk-options')).toBeNull();
		expect(document.getElementById('adv-fdk-afterburner')).toBeNull();
		expect(document.getElementById('encoder-inline-option-row')).toBeNull();
	});

	it('applies the settings-dialog afterburner preference to encoding config', async () => {
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

		const { initializeEncoderPanelLogic, setFdkAfterburner } = await import(
			'../encoderPanel/logic'
		);
		renderEncoder();
		initializeEncoderPanelLogic();
		await waitForEncoderOptions();

		await vi.waitFor(() => {
			const config = readEncodingRequestConfig();
			expect(config.encoderSettings.encoderType).toBe('auto');
			expect(config.encoderSettings.afterburner).toBe(true);
			expect(document.getElementById('encoder-availability-hint')?.textContent).toContain(
				'Afterburner on.',
			);
		});

		setFdkAfterburner(false);

		await vi.waitFor(() => {
			const config = readEncodingRequestConfig();
			expect(config.encoderSettings.encoderType).toBe('auto');
			expect(config.encoderSettings.afterburner).toBe(false);
			expect(document.getElementById('encoder-availability-hint')?.textContent).toContain(
				'Afterburner off.',
			);
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
		renderEncoder();
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

	it('shows compact auto sample-rate and channel resolutions', async () => {
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
		await waitForEncoderOptions();

		renderAutoResolutionHints([
			{
				path: '/books/source.m4b',
				isValid: true,
				sampleRate: 44100,
				channels: 2,
			},
		]);

		await vi.waitFor(() => {
			expect(document.querySelector('[data-testid="auto-samplerate-hint"]')?.textContent).toBe(
				'Auto -> 44.1 kHz',
			);
			expect(document.querySelector('[data-testid="auto-channels-hint"]')?.textContent).toBe(
				'Auto -> Stereo',
			);
		});

		changeSelectValue(document.getElementById('output-samplerate') as HTMLSelectElement, '44100');
		changeSelectValue(document.getElementById('output-channels') as HTMLSelectElement, 'mono');

		await vi.waitFor(() => {
			expect(document.querySelector('[data-testid="auto-samplerate-hint"]')?.textContent).toBe(
				'Using 44100 Hz.',
			);
			expect(document.querySelector('[data-testid="auto-channels-hint"]')?.textContent).toBe(
				'Using Mono.',
			);
		});
	});

	it('retains the session afterburner opt-out across re-init', async () => {
		encoderPanelState.flavor = 'fdk_he_aac';
		encoderPanelState.fdkAfterburner = false;
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
		await waitForEncoderOptions();

		await vi.waitFor(() => {
			expect(encoderPanelState.fdkAfterburner).toBe(false);
		});

		initializeEncoderPanelLogic();
		await vi.waitFor(() => {
			expect(encoderPanelState.fdkAfterburner).toBe(false);
		});
	});

	it('updates encoding request config when bitrate and channel choices change', async () => {
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
		await waitForEncoderOptions();

		const encoderSelect = document.getElementById('adv-encoder') as HTMLSelectElement;
		changeSelectValue(encoderSelect, 'native_aac');

		await vi.waitFor(() => {
			expect(document.getElementById('output-quality')?.classList.contains('hidden')).toBe(true);
		});

		const bitrateSelect = document.getElementById('output-bitrate') as HTMLSelectElement;
		changeSelectValue(bitrateSelect, '48');

		await vi.waitFor(() => {
			expect(readEncodingRequestConfig().encoderSettings.bitrateKbps).toBe(48);
		});

		const channelsSelect = document.getElementById('output-channels') as HTMLSelectElement;
		changeSelectValue(channelsSelect, 'stereo');

		await vi.waitFor(() => {
			expect(readEncodingRequestConfig().encoderSettings.channels).toBe('stereo');
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
		renderEncoder();
		initializeEncoderPanelLogic();
		await waitForEncoderOptions();

		const qualitySelect = document.getElementById('output-quality') as HTMLSelectElement;
		changeSelectValue(qualitySelect, '5');

		await vi.waitFor(() => {
			expect(document.getElementById('estimated-bitrate')?.textContent).toBe('Est: ~96 kbps');
		});
	});

	it('shows detected FDK availability in the existing encoder hint', async () => {
		context.getRuntimeSettingsCapabilitiesMock.mockResolvedValue(
			runtimeSettingsCapabilitiesFixture({
				encoder: {
					availability: {
						...encoderAvailabilityFixture({
							fdkAvailable: true,
							aacAtAvailable: true,
							nativeAacAvailable: true,
						}),
						fdkSource: 'detected',
						detectedToolchainPath: '/opt/homebrew/Cellar/ffmpeg/8.1.1/bin/ffmpeg',
						statusMessage: 'FDK AAC detected and ready.',
					},
				},
			}),
		);

		const { initializeEncoderPanelLogic } = await import('../encoderPanel/logic');
		renderEncoder();
		initializeEncoderPanelLogic();

		await vi.waitFor(() => {
			expect(document.getElementById('encoder-availability-hint')?.textContent).toContain(
				'Using external FDK AAC via /opt/homebrew/.../bin/ffmpeg. Afterburner on.',
			);
			expect(document.body.textContent).not.toContain('Toolchain');
			expect(readEncodingRequestConfig()).toMatchObject({
				encoderSettings: expect.any(Object),
				sampleRate: expect.anything(),
			});
		});
	});

	it('shows missing FDK availability without exposing an override input', async () => {
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
			const select = document.getElementById('adv-encoder') as HTMLSelectElement | null;
			expect(select?.options[0]?.textContent).toBe('Auto (Apple AAC)');
			expect(document.getElementById('encoder-availability-hint')?.textContent).toContain(
				'Auto will use Apple AAC.',
			);
			expect(document.body.textContent).not.toContain('Toolchain');
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
		renderEncoder();
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
		renderEncoder();
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
