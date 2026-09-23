import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { render } from '@solidjs/testing-library';
import { type AppRuntime, createAppRuntime, AppRuntimeProvider } from '../../app/runtime';

import { EncoderView } from '../encoderPanel/EncoderView';
import { emptyInputSession } from '../../app/inputSession/types';
import {
	encoderAvailabilityFixture,
	runtimeSettingsCapabilitiesFixture,
} from '../../test/fixtures/runtimeSettingsCapabilities';

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
		runtime = createAppRuntime();
		return render(() => (
			<AppRuntimeProvider runtime={runtime!}>
				<EncoderView />
			</AppRuntimeProvider>
		));
	}

	beforeEach(() => {
		context.getRuntimeSettingsCapabilitiesMock.mockReset();
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

		renderEncoder();
		await waitForEncoderOptions();

		await vi.waitFor(() => {
			const config = runtime!.encoding.audioRequest();
			expect(config.settings!.encoderType).toBe('auto');
			expect(config.settings!.afterburner).toBe(true);
			expect(document.getElementById('encoder-availability-hint')?.textContent).toContain(
				'Afterburner on.',
			);
		});

		runtime!.encoding.setAfterburner(false);

		await vi.waitFor(() => {
			const config = runtime!.encoding.audioRequest();
			expect(config.settings!.encoderType).toBe('auto');
			expect(config.settings!.afterburner).toBe(false);
			expect(document.getElementById('encoder-availability-hint')?.textContent).toContain(
				'Afterburner off.',
			);
		});
	});

	it('edits native target bitrate and reveals speed under Advanced', async () => {
		context.getRuntimeSettingsCapabilitiesMock.mockResolvedValue(
			runtimeSettingsCapabilitiesFixture(),
		);
		renderEncoder();
		await waitForEncoderOptions();
		changeSelectValue(document.getElementById('adv-encoder') as HTMLSelectElement, 'native_aac');
		await vi.waitFor(() => expect(document.getElementById('native-speed')).not.toBeNull());
		const advanced = document.querySelector('details.encoder-advanced') as HTMLDetailsElement;
		expect(advanced.open).toBe(false);
		advanced.querySelector('summary')!.click();
		expect(advanced.open).toBe(true);
		changeSelectValue(document.getElementById('native-speed') as HTMLSelectElement, '4');
		const bitrate = document.getElementById('output-bitrate') as HTMLInputElement;
		bitrate.value = '193';
		bitrate.dispatchEvent(new Event('change', { bubbles: true }));
		await vi.waitFor(() => {
			const settings = runtime!.encoding.audioRequest().settings!;
			expect(settings.bitrateMode).toEqual({ mode: 'cbr' });
			expect(settings.bitrateKbps).toBe(193);
			expect(settings.nativeAacSpeed).toBe(4);
			expect(runtime!.encoding.readDefaults().settings).toEqual(settings);
		});
		bitrate.value = '0';
		bitrate.dispatchEvent(new Event('change', { bubbles: true }));
		expect(runtime!.encoding.audioRequest().settings!.bitrateKbps).toBe(193);
		expect(bitrate.value).toBe('193');
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
					encoderConfigurations:
						runtimeSettingsCapabilitiesFixture().encoder.encoderConfigurations.map(
							(configuration) => ({ ...configuration, bitrateKbpsMin: 24, bitrateKbpsMax: 256 }),
						),
					explicitSampleRates: [44100],
					channelOptions: ['auto', 'mono'],
				},
			}),
		);

		renderEncoder();
		await waitForEncoderOptions();

		await vi.waitFor(() => {
			const bitrate = document.getElementById('output-bitrate') as HTMLInputElement;
			const sampleRateValues = Array.from(
				(document.getElementById('output-samplerate') as HTMLSelectElement).options,
			).map((option) => option.value);
			const channelValues = Array.from(
				(document.getElementById('output-channels') as HTMLSelectElement).options,
			).map((option) => option.value);

			expect(bitrate.min).toBe('24');
			expect(bitrate.max).toBe('256');
			expect(sampleRateValues).toEqual(['auto', '44100']);
			expect(channelValues).toEqual(['auto', 'mono']);
		});
	});

	it('keeps default source hints independent of the selected title', async () => {
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

		renderEncoder();
		await waitForEncoderOptions();

		runtime!.input.replaceSession({
			...emptyInputSession(),
			fileList: {
				files: [
					{
						path: '/books/source.m4b',
						isValid: true,
						sampleRate: 44100,
						channels: 2,
					},
				],
				selectedDecoders: [null],
				totalDuration: 0,
				totalSize: 0,
				validCount: 1,
				invalidCount: 0,
			},
			selectedIndices: [0],
			selectedAnchor: 0,
		});

		await vi.waitFor(() => {
			expect(
				(document.getElementById('output-samplerate') as HTMLSelectElement).selectedOptions[0]
					?.textContent,
			).toBe('Auto · Source audio');
			expect(
				(document.getElementById('output-channels') as HTMLSelectElement).selectedOptions[0]
					?.textContent,
			).toBe('Auto · Source audio');
		});

		changeSelectValue(document.getElementById('output-samplerate') as HTMLSelectElement, '44100');
		changeSelectValue(document.getElementById('output-channels') as HTMLSelectElement, 'mono');

		await vi.waitFor(() => {
			expect(
				(document.getElementById('output-samplerate') as HTMLSelectElement).selectedOptions[0]
					?.textContent,
			).toBe('44100 Hz');
			expect(
				(document.getElementById('output-channels') as HTMLSelectElement).selectedOptions[0]
					?.textContent,
			).toBe('Mono');
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

		renderEncoder();
		await waitForEncoderOptions();

		await vi.waitFor(() => {
			expect(document.getElementById('output-quality')?.hidden).toBe(false);
			expect(document.getElementById('output-bitrate')?.hidden).toBe(true);
			expect(document.getElementById('quality-bitrate-label')?.textContent).toBe('Quality');
		});

		const encoderSelect = document.getElementById('adv-encoder') as HTMLSelectElement;
		changeSelectValue(encoderSelect, 'native_aac');

		await vi.waitFor(() => {
			expect(document.getElementById('output-quality')?.hidden).toBe(true);
			expect(document.getElementById('output-bitrate')?.hidden).toBe(false);
			expect(document.getElementById('quality-bitrate-label')?.textContent).toBe('Bitrate (kbps)');
		});

		changeSelectValue(encoderSelect, 'aac_at');

		await vi.waitFor(() => {
			expect(document.getElementById('output-quality')?.hidden).toBe(true);
			expect(document.getElementById('output-bitrate')?.hidden).toBe(false);
			expect(document.getElementById('quality-bitrate-label')?.textContent).toBe('Bitrate (kbps)');
		});

		runtime!.encoding.applyDefaults({
			format: 'm4b',
			intent: 'auto',
			settings: {
				...runtime!.encoding.readDefaults().settings,
				encoderType: 'faac',
				faacProfile: 'he_aac_v1',
				bitrateMode: { mode: 'abr' },
			},
			sampleRate: { explicit: 22050 },
		});

		await vi.waitFor(() => {
			expect(runtime!.encoding.audioRequest().settings!.bitrateMode).toEqual({ mode: 'abr' });
			expect(document.getElementById('output-quality')?.hidden).toBe(true);
			expect(document.getElementById('output-bitrate')?.hidden).toBe(false);
			expect(document.getElementById('quality-bitrate-label')?.textContent).toBe('Bitrate (kbps)');
			expect(document.getElementById('estimated-bitrate')).toBeNull();
			const bitrateInput = document.getElementById('output-bitrate') as HTMLInputElement;
			expect(bitrateInput.type).toBe('number');
			expect(bitrateInput.value).toBe('64');
			const sampleRate = document.getElementById('output-samplerate') as HTMLSelectElement;
			expect(sampleRate.value).toBe('22050');
			expect(
				Array.from(sampleRate.options).find((option) => option.value === '22050')?.disabled,
			).toBe(true);
		});

		const bitrateInput = document.getElementById('output-bitrate') as HTMLInputElement;
		expect(bitrateInput).toHaveAccessibleName('Bitrate (kbps)');
		bitrateInput.value = '48';
		bitrateInput.dispatchEvent(new Event('change', { bubbles: true }));

		await vi.waitFor(() => {
			expect(runtime!.encoding.audioRequest().settings!.bitrateKbps).toBe(48);
		});

		const channelsSelect = document.getElementById('output-channels') as HTMLSelectElement;
		changeSelectValue(channelsSelect, 'stereo');

		await vi.waitFor(() => {
			expect(runtime!.encoding.audioRequest().settings!.channels).toBe('stereo');
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

		renderEncoder();
		await waitForEncoderOptions();

		const qualitySelect = document.getElementById('output-quality') as HTMLSelectElement;
		changeSelectValue(qualitySelect, '5');

		await vi.waitFor(() => {
			expect(document.getElementById('estimated-bitrate')?.textContent).toBe('Est: ~96 kbps');
		});
	});

	it('wires FAAC profile and rate controls to the request and restores the ABR target', async () => {
		context.getRuntimeSettingsCapabilitiesMock.mockResolvedValue(
			runtimeSettingsCapabilitiesFixture(),
		);
		renderEncoder();
		await waitForEncoderOptions();
		changeSelectValue(document.getElementById('adv-encoder') as HTMLSelectElement, 'faac');
		await vi.waitFor(() => expect(document.getElementById('faac-profile')).not.toBeNull());
		const profile = document.getElementById('faac-profile') as HTMLSelectElement;
		const rate = document.getElementById('faac-rate-control') as HTMLSelectElement;
		expect(profile).toHaveAccessibleName('Profile');
		expect(profile.value).toBe('auto');
		expect(rate).toHaveAccessibleName('Rate control');
		expect(rate.value).toBe('abr');
		changeSelectValue(rate, 'vbr');
		await vi.waitFor(() => expect(document.getElementById('output-quality')?.hidden).toBe(false));
		const quality = document.getElementById('output-quality') as HTMLSelectElement;
		expect(Array.from(quality.options).map((option) => option.textContent)).toEqual([
			'Smaller (50)',
			'Standard (100)',
			'Higher (200)',
		]);
		changeSelectValue(quality, '50');
		changeSelectValue(profile, 'aac_lc');
		await vi.waitFor(() => {
			expect(runtime!.encoding.audioRequest().settings).toMatchObject({
				faacProfile: 'aac_lc',
				bitrateMode: { mode: 'vbr', value: 50 },
			});
			expect(document.getElementById('estimated-bitrate')?.textContent).toContain('choose ABR');
		});
		changeSelectValue(rate, 'abr');
		await vi.waitFor(() => {
			expect(document.getElementById('output-bitrate')?.hidden).toBe(false);
			expect((document.getElementById('output-bitrate') as HTMLInputElement).value).toBe('64');
			expect(runtime!.encoding.audioRequest().settings!.faacProfile).toBe('aac_lc');
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

		renderEncoder();

		await vi.waitFor(() => {
			expect(document.getElementById('encoder-availability-hint')?.textContent).toContain(
				'Using external FDK AAC via /opt/homebrew/.../bin/ffmpeg. Afterburner on.',
			);
			expect(document.body.textContent).not.toContain('Toolchain');
			expect(runtime!.encoding.audioRequest()).toMatchObject({
				settings: expect.any(Object),
				sampleRate: expect.anything(),
			});
		});
	});

	it('normalizes a session-selected unavailable Apple AAC flavor back to auto', async () => {
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

		renderEncoder();
		await waitForEncoderOptions();
		runtime!.encoding.select('encoder', 'aac_at');

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
		await runtime!.encoding.reloadCapabilities();

		await vi.waitFor(() => {
			const select = document.getElementById('adv-encoder') as HTMLSelectElement | null;
			expect(select?.value).toBe('auto');
			expect(select?.options[0]?.textContent).toBe('App default (Native AAC (NMR))');
			expect(document.getElementById('encoder-availability-hint')?.textContent).toContain(
				'Auto will use Native AAC (NMR).',
			);
			expect(runtime!.encoding.audioRequest().settings!.encoderType).toBe('auto');
		});
	});

	it('normalizes a session-selected unavailable native AAC flavor back to auto', async () => {
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

		renderEncoder();
		await waitForEncoderOptions();
		runtime!.encoding.select('encoder', 'native_aac');

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
		await runtime!.encoding.reloadCapabilities();

		await vi.waitFor(() => {
			const select = document.getElementById('adv-encoder') as HTMLSelectElement | null;
			expect(select?.value).toBe('auto');
			expect(select?.options[0]?.textContent).toBe('App default (Apple AAC)');
			expect(document.getElementById('encoder-availability-hint')?.textContent).toBe(
				'Auto will use Apple AAC. FDK AAC is not available. Set up FDK…',
			);
			expect(runtime!.encoding.audioRequest().settings!.encoderType).toBe('auto');
		});
	});
});
