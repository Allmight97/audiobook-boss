import { afterEach, describe, expect, it, vi } from 'vitest';
import { render, screen } from '@solidjs/testing-library';
import { type AppRuntime, createAppRuntime, AppRuntimeProvider } from '../../app/runtime';
import { createFakeEngine, type FakeEngine } from '../../test/fixtures/fakeEngine';
import type { AudioChoiceView, SessionAudio } from '../../types/session';
import { EncoderView } from '../encoderPanel';

// The engine decides what each edit does; these tests seed the engine's
// choice and check what the panel shows and which edit it sends.

const changeSelectValue = (select: HTMLSelectElement, value: string): void => {
	select.value = value;
	select.dispatchEvent(new Event('change', { bubbles: true }));
};

describe('encoder panel behavior controls', () => {
	let runtime: AppRuntime | undefined;
	let engine: FakeEngine;

	afterEach(() => {
		runtime?.dispose();
		runtime = undefined;
	});

	async function renderEncoder(seed: (defaults: AudioChoiceView, audio: SessionAudio) => void) {
		engine = createFakeEngine();
		engine.change((state) => {
			state.audio.defaults.choice.intent = 'encode';
			seed(state.audio.defaults, state.audio);
		});
		runtime = createAppRuntime({ engine });
		await runtime.initialize();
		return render(() => (
			<AppRuntimeProvider runtime={runtime!}>
				<EncoderView />
			</AppRuntimeProvider>
		));
	}

	function sent() {
		return engine.sessionIntents.filter((intent) => intent.kind === 'setDefaultAudio');
	}

	it('hides encoder controls for Default and Keep original audio', async () => {
		await renderEncoder(() => undefined);
		await vi.waitFor(() => expect(screen.getByLabelText('Encoder')).toBeTruthy());

		for (const intent of ['auto', 'preserve'] as const) {
			engine.change((state) => {
				state.audio.defaults.choice.intent = intent;
			});
			await vi.waitFor(() => expect(screen.queryByLabelText('Encoder')).toBeNull());
		}
		changeSelectValue(screen.getByLabelText('Audio handling') as HTMLSelectElement, 'encode');
		await vi.waitFor(() =>
			expect(sent().slice(-1)[0]).toEqual({
				kind: 'setDefaultAudio',
				edit: { field: 'intent', value: 'encode' },
			}),
		);
	});

	it('sends native speed and bitrate edits and shows the engine value after a refused one', async () => {
		await renderEncoder((defaults) => {
			defaults.choice.aacBitrateKbps = 193;
		});
		await vi.waitFor(() => expect(document.getElementById('native-speed')).not.toBeNull());

		changeSelectValue(document.getElementById('native-speed') as HTMLSelectElement, '4');
		const bitrate = document.getElementById('output-bitrate') as HTMLInputElement;
		bitrate.value = '0';
		bitrate.dispatchEvent(new Event('change', { bubbles: true }));

		await vi.waitFor(() =>
			expect(sent()).toEqual([
				{ kind: 'setDefaultAudio', edit: { field: 'nativeSpeed', value: 4 } },
				{ kind: 'setDefaultAudio', edit: { field: 'bitrate', value: 0 } },
			]),
		);
		// The engine refused 0, so the panel still shows its value.
		engine.change(() => undefined);
		await vi.waitFor(() => expect(bitrate.value).toBe('193'));
	});

	it('renders option ranges from the engine facts and capabilities', async () => {
		await renderEncoder((defaults, audio) => {
			defaults.facts.bitrateKbpsMin = 24;
			defaults.facts.bitrateKbpsMax = 256;
			if (audio.capabilities) {
				audio.capabilities.explicitSampleRates = [44100];
				audio.capabilities.channelOptions = ['auto', 'mono'];
			}
		});

		await vi.waitFor(() => {
			const bitrate = document.getElementById('output-bitrate') as HTMLInputElement;
			const values = (id: string) =>
				Array.from((document.getElementById(id) as HTMLSelectElement).options).map(
					(option) => option.value,
				);
			expect(bitrate.min).toBe('24');
			expect(bitrate.max).toBe('256');
			expect(values('output-samplerate')).toEqual(['auto', '44100']);
			expect(values('output-channels')).toEqual(['auto', 'mono']);
		});
	});

	it('keeps default source hints independent of the selected title', async () => {
		await renderEncoder(() => undefined);
		engine.loadTitles(
			[{ path: '/books/source.m4b', isValid: true, sampleRate: 44100, channels: 2 }],
			[0],
		);

		await vi.waitFor(() => {
			const shown = (id: string) =>
				(document.getElementById(id) as HTMLSelectElement).selectedOptions[0]?.textContent;
			expect(shown('output-samplerate')).toBe('Auto · Source audio');
			expect(shown('output-channels')).toBe('Auto · Source audio');
		});
	});

	it('shows FAAC quality presets and a kept rate the encoder rejects', async () => {
		await renderEncoder((defaults) => {
			defaults.choice.encoder = 'faac';
			defaults.choice.faacRateControl = 'vbr';
			defaults.choice.sampleRate = { explicit: 22050 };
			defaults.facts = {
				...defaults.facts,
				effectiveEncoder: 'faac',
				bitrateMode: { mode: 'vbr', value: 100 },
				allowedModes: ['abr', 'vbr'],
				faacProfiles: ['auto', 'aac_lc', 'he_aac_v1'],
				allowedSampleRates: [44100],
				sampleRateSupported: false,
				estimateKbps: null,
			};
		});

		await vi.waitFor(() => expect(document.getElementById('output-quality')?.hidden).toBe(false));
		const quality = document.getElementById('output-quality') as HTMLSelectElement;
		expect(Array.from(quality.options).map((option) => option.textContent)).toEqual([
			'Smaller (50)',
			'Standard (100)',
			'Higher (200)',
		]);
		const profile = document.getElementById('faac-profile') as HTMLSelectElement;
		expect(profile.selectedOptions[0]?.textContent).toBe('Auto · FAAC chooses LC or HE');
		const sampleRate = document.getElementById('output-samplerate') as HTMLSelectElement;
		expect(sampleRate.value).toBe('22050');
		expect(
			Array.from(sampleRate.options).find((option) => option.value === '22050')?.disabled,
		).toBe(true);

		changeSelectValue(quality, '50');
		changeSelectValue(document.getElementById('faac-rate-control') as HTMLSelectElement, 'abr');
		await vi.waitFor(() =>
			expect(sent()).toEqual([
				{ kind: 'setDefaultAudio', edit: { field: 'quality', value: 50 } },
				{ kind: 'setDefaultAudio', edit: { field: 'rateControl', value: 'abr' } },
			]),
		);
	});

	it('keeps showing a chosen encoder that is no longer available', async () => {
		await renderEncoder((defaults, audio) => {
			defaults.choice.encoder = 'aac_at';
			defaults.facts.effectiveEncoder = 'aac_at';
			if (audio.capabilities) audio.capabilities.availability.aacAtAvailable = false;
			defaults.facts.encoderOptions = defaults.facts.encoderOptions.map((option) =>
				option.encoder === 'aac_at' ? { ...option, available: false } : option,
			);
		});

		await vi.waitFor(() => {
			const select = document.getElementById('adv-encoder') as HTMLSelectElement | null;
			expect(select?.value).toBe('aac_at');
			expect(
				Array.from(select?.options ?? []).find((option) => option.value === 'aac_at')?.disabled,
			).toBe(true);
		});
	});
});
