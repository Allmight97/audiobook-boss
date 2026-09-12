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
				input: { view: inputView },
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

	it('hydrates VBR request without persisting, then persists only on select', async () => {
		mounted = mountEncoding();
		await ready(mounted.owner);

		mounted.owner.applyDefaults(vbrDefaults(4));
		flush();
		expect(mounted.owner.request().encoderSettings.bitrateMode).toEqual({ mode: 'vbr', value: 4 });
		expect(mounted.owner.estimateKbps()).toBe(72);
		expect(mounted.persist).not.toHaveBeenCalled();

		mounted.owner.select('quality', '2');
		flush();
		expect(mounted.owner.estimateKbps()).toBe(48);
		expect(mounted.owner.request().encoderSettings.bitrateKbps).toBe(64);
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
			expect(mounted.owner.request().encoderSettings.bitrateMode).toEqual({ mode });
			expect(mounted.owner.view().showQuality).toBe(false);
			finish(encoderCaps());
			await ready(mounted.owner);
			expect(mounted.owner.request().encoderSettings.bitrateMode).toEqual({ mode });
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
		expect(mounted.owner.request().encoderSettings.bitrateMode).toEqual({ mode: 'cbr' });
		expect(mounted.owner.request().encoderSettings.bitrateKbps).toBe(96);
		expect(mounted.owner.estimateKbps()).toBe(96);
	});

	it('derives each encoder mode while retaining quality, target and native speed', async () => {
		mounted = mountEncoding();
		await ready(mounted.owner);
		mounted.owner.select('quality', '4');
		mounted.owner.select('encoder', 'native_aac');
		mounted.owner.select('nativeSpeed', '4');
		mounted.owner.select('bitrate', '193');
		expect(mounted.owner.request().encoderSettings.bitrateMode).toEqual({ mode: 'cbr' });
		mounted.owner.select('encoder', 'aac_at');
		expect(mounted.owner.request().encoderSettings).toMatchObject({
			bitrateMode: { mode: 'cvbr' },
			bitrateKbps: 193,
			nativeAacSpeed: 4,
		});
		mounted.owner.select('encoder', 'fdk_he_aac');
		expect(mounted.owner.request().encoderSettings.bitrateMode).toEqual({ mode: 'vbr', value: 4 });
		mounted.owner.select('encoder', 'native_aac');
		expect(mounted.owner.readDefaults().settings).toMatchObject({
			bitrateMode: { mode: 'cbr' },
			bitrateKbps: 193,
			nativeAacSpeed: 4,
		});
	});

	it('clamps hydrated controls and capability changes without persisting', async () => {
		const capabilities = encoderCaps({
			bitrateKbpsMin: 24,
			bitrateKbpsMax: 256,
			nativeSpeedMax: 2,
			vbrLevelMin: 2,
			vbrLevelMax: 4,
			explicitSampleRates: [22050],
			channelOptions: ['mono'],
		});
		mounted = mountEncoding({ capabilities });
		mounted.owner.select('bitrate', '1000');
		expect(mounted.owner.request().encoderSettings.bitrateKbps).toBe(64);
		await ready(mounted.owner);
		const defaults = vbrDefaults(5);
		defaults.settings.bitrateKbps = 1000;
		defaults.settings.nativeAacSpeed = 4;
		defaults.settings.channels = 'stereo';
		defaults.sampleRate = { explicit: 44100 };
		mounted.owner.applyDefaults(defaults);
		expect(mounted.owner.request()).toMatchObject({
			encoderSettings: {
				bitrateKbps: 256,
				bitrateMode: { mode: 'vbr', value: 4 },
				nativeAacSpeed: 2,
				channels: 'mono',
			},
			sampleRate: 'auto',
		});
		await mounted.owner.reloadCapabilities({
			...capabilities,
			bitrateKbpsMax: 128,
			nativeSpeedMax: 1,
		});
		expect(mounted.owner.request().encoderSettings).toMatchObject({
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
		expect(mounted.owner.request().encoderSettings.encoderType).toBe('auto');
		expect(mounted.owner.request().encoderSettings.bitrateMode).toEqual({ mode: 'cbr' });
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
		expect(mounted.owner.request().encoderSettings.afterburner).toBe(false);
		mounted.persist.mockClear();

		await mounted.owner.reloadCapabilities();
		flush();
		expect(mounted.owner.request().encoderSettings.afterburner).toBe(false);
		expect(mounted.persist).not.toHaveBeenCalled();
	});

	it('derives auto-resolution hints from selected Input files', async () => {
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
		expect(mounted.owner.view().sampleRateHint).toBe('Auto -> 44.1 kHz');
		expect(mounted.owner.view().channelsHint).toBe('Auto -> Stereo');
		mounted.owner.select('channels', 'stereo');
		flush();
		expect(mounted.owner.view().channelsHint).toBe('Using Stereo.');
		mounted.setInput(
			emptyInputView({
				files: [{ ...file, channels: 6 }],
				fileCount: 1,
				hasFiles: true,
				selectedIndices: [0],
			}),
		);
		flush();
		expect(mounted.owner.view().channelsHint).toBe(
			'Using Stereo. Surround downmix omits bass effects (LFE).',
		);
		mounted.owner.select('channels', 'auto');
		flush();
		expect(mounted.owner.view().channelsHint).toBe(
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
		expect(first.owner.request().encoderSettings.encoderType).toBe('native_aac');
		expect(second.owner.request().encoderSettings.encoderType).toBe('auto');
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
