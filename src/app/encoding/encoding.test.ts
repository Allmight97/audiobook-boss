import { createRoot, createSignal, flush, runWithOwner } from 'solid-js';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { EncoderDefaults } from '../../types/appSettings';
import type { AudioFile, EncoderSettingsCapabilities } from '../../types/audio';
import { runtimeSettingsCapabilitiesFixture } from '../../test/fixtures/runtimeSettingsCapabilities';
import type { InputView } from '../inputSession';
import { createEncodingOwner, type EncodingOwner } from './owner';

function emptyInputView(overrides: Partial<InputView> = {}): InputView {
	return {
		files: [],
		sourceFiles: [],
		selectedSourceFiles: (overrides.selectedIndices ?? []).flatMap(
			(index) => overrides.files?.[index] ?? [],
		),
		selectedIndices: [],
		selectedAnchor: -1,
		fileCount: 0,
		hasFiles: false,
		orderLocked: false,
		errorMessage: '',
		isDragOver: false,
		supportText: '',
		sortDirection: 'none',
		sortLabel: 'Sort: A-Z',
		orderDiffersFromImport: false,
		showSortButton: false,
		showClearButton: false,
		showRestoreImportOrder: false,
		totalDurationSeconds: 0,
		...overrides,
	};
}

function encoderCaps(
	overrides: Partial<EncoderSettingsCapabilities> = {},
): EncoderSettingsCapabilities {
	return {
		...runtimeSettingsCapabilitiesFixture().encoder,
		...overrides,
	};
}

function vbrDefaults(value: number, afterburner = true): EncoderDefaults {
	return {
		format: 'm4b',
		intent: 'auto',
		settings: {
			encoderType: 'auto',
			bitrateKbps: 64,
			bitrateMode: { mode: 'vbr', value },
			channels: 'auto',
			afterburner,
		},
		sampleRate: 'auto',
	};
}

type Mounted = {
	readonly owner: EncodingOwner;
	readonly persist: ReturnType<typeof vi.fn>;
	setInput(view: InputView): void;
	dispose(): void;
};

function mountEncoding(
	options: {
		readonly capabilities?: EncoderSettingsCapabilities | null;
		readonly load?: () => Promise<EncoderSettingsCapabilities | null>;
	} = {},
): Mounted {
	return runWithOwner(null, () =>
		createRoot((dispose) => {
			const persist = vi.fn();
			const [inputView, setInputView] = createSignal(emptyInputView(), { ownedWrite: true });
			const owner = createEncodingOwner({
				input: {
					sourcesFor: (file) => [file],
					view: inputView,
					audioRequest: () => undefined,
					setAudioRequest: vi.fn(),
				},
				loadCapabilities: options.load ?? (async () => options.capabilities ?? encoderCaps()),
				persistDefaults: persist,
			});
			return {
				owner,
				persist,
				setInput(view: InputView) {
					setInputView(view);
				},
				dispose,
			};
		}),
	);
}

describe('encoding owner', () => {
	let mounted: Mounted | undefined;

	afterEach(() => {
		mounted?.dispose();
		mounted = undefined;
	});

	async function ready(owner: EncodingOwner): Promise<void> {
		await vi.waitFor(() => {
			expect(owner.view().flavorOptions.length).toBeGreaterThan(1);
		});
		flush();
	}

	it.each([true, false])(
		'identifies bundled FAAC regardless of Apple availability (%s)',
		async (aacAtAvailable) => {
			const capabilities = encoderCaps();
			mounted = mountEncoding({
				capabilities: {
					...capabilities,
					availability: { ...capabilities.availability, aacAtAvailable },
				},
			});
			await ready(mounted.owner);
			mounted.owner.select('encoder', 'faac');
			flush();
			expect(mounted.owner.view().availabilityHint).toBe('FAAC is included with ABB.');
		},
	);

	it('hydrates FAAC as an explicit ABR encoder and preserves an unsupported rate', async () => {
		mounted = mountEncoding();
		await ready(mounted.owner);

		mounted.owner.applyDefaults({
			format: 'm4b',
			intent: 'auto',
			settings: {
				...vbrDefaults(3).settings,
				encoderType: 'faac',
				faacProfile: 'he_aac_v1',
				bitrateMode: { mode: 'abr' },
			},
			sampleRate: { explicit: 22050 },
		});
		flush();

		expect(mounted.owner.audioRequest().settings).toMatchObject({
			encoderType: 'faac',
			faacProfile: 'he_aac_v1',
			bitrateMode: { mode: 'abr' },
		});
		expect(mounted.owner.audioRequest().sampleRate).toEqual({ explicit: 22050 });
		expect(mounted.owner.view().showQuality).toBe(false);
		expect(mounted.owner.view().qualityBitrateLabel).toBe('Bitrate (kbps)');
		expect(mounted.owner.view().estimatedBitrateText).toBe('');
		expect(
			mounted.owner.view().sampleRateOptions.find((option) => option.value === '22050')?.disabled,
		).toBe(true);
		expect(mounted.owner.view().sampleRateHint).toBe(
			'Choose a supported sample rate for this encoder.',
		);

		mounted.owner.select('encoder', 'native_aac');
		mounted.owner.select('encoder', 'faac');
		await mounted.owner.reloadCapabilities(encoderCaps());
		flush();
		expect(mounted.owner.audioRequest().sampleRate).toEqual({ explicit: 22050 });
		expect(mounted.owner.view().sampleRateHint).toBe(
			'Choose a supported sample rate for this encoder.',
		);

		mounted.owner.select('sampleRate', '48000');
		flush();
		expect(mounted.owner.audioRequest().sampleRate).toEqual({ explicit: 48000 });
	});

	it('keeps FAAC profile and rate choices independent of source Auto and FDK quality', async () => {
		mounted = mountEncoding();
		await ready(mounted.owner);
		mounted.owner.select('encoder', 'faac');
		expect(mounted.owner.audioRequest()).toMatchObject({
			sampleRate: 'auto',
			settings: {
				faacProfile: 'auto',
				channels: 'auto',
				bitrateKbps: 64,
				bitrateMode: { mode: 'abr' },
			},
		});
		mounted.owner.select('rateControl', 'vbr');
		mounted.owner.select('quality', '200');
		mounted.owner.select('faacProfile', 'aac_lc');
		mounted.owner.select('encoder', 'fdk_he_aac');
		expect(mounted.owner.audioRequest().settings!.bitrateMode).toEqual({ mode: 'vbr', value: 3 });
		mounted.owner.select('quality', '4');
		mounted.owner.select('encoder', 'faac');
		expect(mounted.owner.audioRequest().settings).toMatchObject({
			faacProfile: 'aac_lc',
			bitrateMode: { mode: 'vbr', value: 200 },
		});
		mounted.owner.select('rateControl', 'abr');
		mounted.owner.select('sampleRate', '22050');
		expect(mounted.owner.audioRequest().sampleRate).toEqual({ explicit: 22050 });
		mounted.owner.select('faacProfile', 'he_aac_v1');
		expect(mounted.owner.audioRequest().sampleRate).toEqual({ explicit: 22050 });
		expect(mounted.owner.view().sampleRateHint).toContain('Choose a supported');
		mounted.owner.select('faacProfile', 'auto');
		expect(
			mounted.owner.view().sampleRateOptions.find((option) => option.value === '22050')?.disabled,
		).toBe(false);
		mounted.owner.select('rateControl', 'vbr');
		expect(mounted.owner.audioRequest().settings!.bitrateMode).toEqual({
			mode: 'vbr',
			value: 200,
		});
		const saved = mounted.owner.readDefaults();
		mounted.owner.applyDefaults(saved);
		await mounted.owner.reloadCapabilities(encoderCaps());
		expect(mounted.owner.audioRequest().settings!.bitrateMode).toEqual({
			mode: 'vbr',
			value: 200,
		});
	});

	it('hydrates VBR request without persisting, then persists only on select', async () => {
		mounted = mountEncoding();
		await ready(mounted.owner);

		mounted.owner.applyDefaults(vbrDefaults(4));
		flush();
		expect(mounted.owner.audioRequest().settings!.bitrateMode).toEqual({ mode: 'vbr', value: 4 });
		expect(mounted.persist).not.toHaveBeenCalled();

		mounted.owner.select('quality', '2');
		flush();
		expect(mounted.owner.audioRequest().settings!.bitrateKbps).toBe(64);
		expect(mounted.persist).toHaveBeenCalledTimes(1);
		expect(mounted.persist.mock.calls[0]?.[0].settings.bitrateMode).toEqual({
			mode: 'vbr',
			value: 2,
		});
	});

	it.each([
		{ encoderType: 'native_aac', mode: 'cbr' },
		{ encoderType: 'aac_at', mode: 'cvbr' },
	] as const)(
		'waits for discovery before resolving Auto to $encoderType',
		async ({ encoderType, mode }) => {
			let finish!: (capabilities: EncoderSettingsCapabilities) => void;
			const pending = new Promise<EncoderSettingsCapabilities>((resolve) => {
				finish = resolve;
			});
			mounted = mountEncoding({ load: () => pending });
			mounted.owner.applyDefaults(vbrDefaults(3));
			expect(mounted.owner.audioRequest().settings?.encoderType).toBe('auto');
			expect(mounted.owner.readDefaults().settings.encoderType).toBe('auto');
			finish(
				encoderCaps({
					availability: {
						...encoderCaps().availability,
						fdkAvailable: false,
						aacAtAvailable: encoderType === 'aac_at',
						autoEncoder: encoderType,
					},
				}),
			);
			await ready(mounted.owner);
			expect(mounted.owner.audioRequest().settings!.bitrateMode).toEqual({ mode });
			expect(mounted.persist).not.toHaveBeenCalled();
		},
	);

	it('keeps unavailable controls disabled after discovery fails and recovers on reload', async () => {
		const warn = vi.spyOn(console, 'warn').mockImplementation(() => undefined);
		try {
			mounted = mountEncoding({ load: async () => Promise.reject(new Error('Scan failed')) });
			await vi.waitFor(() => expect(warn).toHaveBeenCalled());
			expect(mounted.owner.view().flavorDisabled).toBe(true);
			await mounted.owner.reloadCapabilities(encoderCaps());
			expect(mounted.owner.audioRequest().settings!.bitrateMode).toEqual({
				mode: 'vbr',
				value: 3,
			});
		} finally {
			warn.mockRestore();
		}
	});

	it.each([
		{ encoderType: 'native_aac', mode: 'cbr' },
		{ encoderType: 'aac_at', mode: 'cvbr' },
	] as const)(
		'keeps hydrated $encoderType intent while discovery is pending',
		async ({ encoderType, mode }) => {
			let finish!: (capabilities: EncoderSettingsCapabilities) => void;
			const pending = new Promise<EncoderSettingsCapabilities>((resolve) => {
				finish = resolve;
			});
			mounted = mountEncoding({ load: () => pending });
			mounted.owner.applyDefaults({
				...vbrDefaults(3),
				settings: { ...vbrDefaults(3).settings, encoderType, bitrateMode: { mode } },
			});
			expect(mounted.owner.audioRequest().settings!.bitrateMode).toEqual({ mode });
			expect(mounted.owner.view().showQuality).toBe(false);
			finish(encoderCaps());
			await ready(mounted.owner);
			expect(mounted.owner.audioRequest().settings!.bitrateMode).toEqual({ mode });
			expect(mounted.persist).not.toHaveBeenCalled();
		},
	);

	it('does not treat sticky CBR bitrate as VBR quality', async () => {
		mounted = mountEncoding();
		await ready(mounted.owner);

		mounted.owner.select('encoder', 'native_aac');
		flush();
		expect(mounted.owner.view().showQuality).toBe(false);
		mounted.owner.select('bitrate', '96');
		flush();
		expect(mounted.owner.audioRequest().settings!.bitrateMode).toEqual({ mode: 'cbr' });
		expect(mounted.owner.audioRequest().settings!.bitrateKbps).toBe(96);
	});

	it('derives each encoder mode while retaining quality, target and native speed', async () => {
		mounted = mountEncoding();
		await ready(mounted.owner);
		mounted.owner.select('quality', '4');
		mounted.owner.select('encoder', 'native_aac');
		mounted.owner.select('nativeSpeed', '4');
		mounted.owner.select('bitrate', '193');
		expect(mounted.owner.audioRequest().settings!.bitrateMode).toEqual({ mode: 'cbr' });
		mounted.owner.select('encoder', 'aac_at');
		expect(mounted.owner.audioRequest().settings).toMatchObject({
			bitrateMode: { mode: 'cvbr' },
			bitrateKbps: 193,
			nativeAacSpeed: 4,
		});
		mounted.owner.select('encoder', 'fdk_he_aac');
		expect(mounted.owner.audioRequest().settings!.bitrateMode).toEqual({ mode: 'vbr', value: 4 });
		mounted.owner.select('encoder', 'native_aac');
		expect(mounted.owner.readDefaults().settings).toMatchObject({
			bitrateMode: { mode: 'cbr' },
			bitrateKbps: 193,
			nativeAacSpeed: 4,
		});
	});

	it('clamps hydrated controls and capability changes without persisting', async () => {
		const capabilities = encoderCaps({
			encoderConfigurations: encoderCaps().encoderConfigurations.map((configuration) => ({
				...configuration,
				bitrateKbpsMin: 24,
				bitrateKbpsMax: 256,
			})),
			nativeSpeedMax: 2,
			vbrLevelMin: 2,
			vbrLevelMax: 4,
			explicitSampleRates: [22050],
			channelOptions: ['mono'],
		});
		mounted = mountEncoding({ capabilities });
		mounted.owner.select('bitrate', '1000');
		expect(mounted.owner.readDefaults().settings.bitrateKbps).toBe(64);
		await ready(mounted.owner);
		const defaults = vbrDefaults(5);
		defaults.settings.bitrateKbps = 1000;
		defaults.settings.nativeAacSpeed = 4;
		defaults.settings.channels = 'stereo';
		defaults.sampleRate = { explicit: 44100 };
		mounted.owner.applyDefaults(defaults);
		expect(mounted.owner.audioRequest()).toMatchObject({
			settings: {
				bitrateKbps: 256,
				bitrateMode: { mode: 'vbr', value: 4 },
				nativeAacSpeed: 2,
				channels: 'mono',
			},
			sampleRate: 'auto',
		});
		await mounted.owner.reloadCapabilities({
			...capabilities,
			encoderConfigurations: capabilities.encoderConfigurations.map((configuration) => ({
				...configuration,
				bitrateKbpsMax: 128,
			})),
			nativeSpeedMax: 1,
		});
		expect(mounted.owner.audioRequest().settings).toMatchObject({
			bitrateKbps: 128,
			nativeAacSpeed: 1,
		});
		expect(mounted.persist).not.toHaveBeenCalled();
	});

	it('snaps an unavailable explicit flavor to auto without persisting', async () => {
		mounted = mountEncoding({
			capabilities: encoderCaps({
				availability: {
					...runtimeSettingsCapabilitiesFixture().encoder.availability,
					fdkAvailable: false,
					aacAtAvailable: false,
					nativeAacAvailable: true,
					autoEncoder: 'native_aac',
				},
			}),
		});
		await ready(mounted.owner);

		mounted.persist.mockClear();
		mounted.owner.select('encoder', 'aac_at');
		flush();
		expect(mounted.owner.audioRequest().settings!.encoderType).toBe('auto');
		expect(mounted.owner.audioRequest().settings!.bitrateMode).toEqual({ mode: 'cbr' });
		expect(mounted.persist).not.toHaveBeenCalled();
	});

	it('keeps a shared Settings scan when an older startup scan finishes later', async () => {
		let finish!: (capabilities: EncoderSettingsCapabilities) => void;
		const pending = new Promise<EncoderSettingsCapabilities>((resolve) => {
			finish = resolve;
		});
		const load = vi.fn(() => pending);
		mounted = mountEncoding({ load });
		const missing = encoderCaps({
			availability: { ...encoderCaps().availability, fdkAvailable: false },
		});
		await mounted.owner.reloadCapabilities(missing);
		expect(mounted.owner.view().fdkSetupNeeded).toBe(true);
		finish(encoderCaps());
		await pending;
		flush();
		expect(mounted.owner.view().fdkSetupNeeded).toBe(true);
		expect(load).toHaveBeenCalledTimes(1);
		expect(mounted.persist).not.toHaveBeenCalled();
	});

	it('keeps afterburner across a capability reload', async () => {
		mounted = mountEncoding();
		await ready(mounted.owner);
		mounted.owner.setAfterburner(false);
		flush();
		expect(mounted.owner.audioRequest().settings!.afterburner).toBe(false);
		mounted.persist.mockClear();

		await mounted.owner.reloadCapabilities();
		flush();
		expect(mounted.owner.audioRequest().settings!.afterburner).toBe(false);
		expect(mounted.persist).not.toHaveBeenCalled();
	});

	it('derives audio hints from the title while defaults stay independent of selection', async () => {
		mounted = mountEncoding();
		await ready(mounted.owner);
		const file: AudioFile = {
			path: '/books/source.m4b',
			isValid: true,
			sampleRate: 44100,
			channels: 2,
		};
		mounted.setInput(
			emptyInputView({
				files: [file],
				fileCount: 1,
				hasFiles: true,
				selectedIndices: [0],
				selectedAnchor: 0,
			}),
		);
		flush();
		expect(mounted.owner.titleView(file).sampleRateHint).toBe('Auto -> 44.1 kHz');
		expect(mounted.owner.titleView(file).channelsHint).toBe('Auto -> Stereo');
		mounted.owner.select('channels', 'stereo');
		flush();
		expect(mounted.owner.titleView(file).channelsHint).toBe('Using Stereo.');
		file.channels = 6;
		mounted.setInput(
			emptyInputView({
				files: [file],
				fileCount: 1,
				hasFiles: true,
				selectedIndices: [0],
			}),
		);
		flush();
		expect(mounted.owner.titleView(file).channelsHint).toBe(
			'Using Stereo. Surround downmix omits bass effects (LFE).',
		);
		mounted.owner.select('channels', 'auto');
		flush();
		expect(mounted.owner.titleView(file).channelsHint).toBe(
			'Multichannel input: choose Mono or Stereo to downmix.',
		);
	});

	it('isolates two owners with separate bags and persist adapters', async () => {
		const first = mountEncoding();
		const second = mountEncoding();
		mounted = {
			owner: first.owner,
			persist: first.persist,
			setInput: first.setInput,
			dispose() {
				first.dispose();
				second.dispose();
			},
		};
		await ready(first.owner);
		await ready(second.owner);

		first.owner.select('encoder', 'native_aac');
		flush();
		expect(first.owner.audioRequest().settings!.encoderType).toBe('native_aac');
		expect(second.owner.audioRequest().settings!.encoderType).toBe('auto');
		expect(second.persist).not.toHaveBeenCalled();
	});

	it('drops a late capability load after reset', async () => {
		let resolveLoad!: (value: EncoderSettingsCapabilities | null) => void;
		const load = () =>
			new Promise<EncoderSettingsCapabilities | null>((resolve) => {
				resolveLoad = resolve;
			});
		mounted = mountEncoding({ load });
		mounted.owner.reset();
		resolveLoad(encoderCaps());
		await Promise.resolve();
		flush();
		expect(mounted.owner.view().flavorOptions).toEqual([
			{ value: 'auto', label: 'Loading…', disabled: true },
		]);
	});
});
