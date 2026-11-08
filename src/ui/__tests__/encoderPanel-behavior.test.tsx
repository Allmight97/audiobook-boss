import { afterEach, describe, expect, it, vi } from 'vitest';
import { render, screen } from '@solidjs/testing-library';
import { type AppRuntime, createAppRuntime, AppRuntimeProvider } from '../../app/runtime';
import { audioFile, createFakeEngine, type FakeEngine } from '../../test/fixtures/fakeEngine';
import type { AudioChoiceView, AudioSnapshot } from '../../lib/generated/tauri';
import { EncoderView } from '../encoderPanel';

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

	async function renderEncoder(seed: (defaults: AudioChoiceView, audio: AudioSnapshot) => void) {
		engine = createFakeEngine();
		engine.change((state) => {
			state.audio.defaults.choice.intent = 'encode';
			seed(state.audio.defaults, state.audio);
		});
		runtime = await createAppRuntime({ engine });
		return render(() => (
			<AppRuntimeProvider runtime={runtime!}>
				<EncoderView scope={{ kind: 'defaults' }} />
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
			// oxlint-disable-next-line eslint/no-await-in-loop -- each intent must render before the next one is set.
			await vi.waitFor(() => expect(screen.queryByLabelText('Encoder')).toBeNull());
		}
		changeSelectValue(screen.getByLabelText('Audio handling'), 'encode');
		await vi.waitFor(() =>
			expect(sent().slice(-1)[0]).toEqual({
				kind: 'setDefaultAudio',
				edit: { field: 'intent', value: 'encode' },
			}),
		);
	});

	it('sends native speed and bitrate edits and shows the engine value after a refused one', async () => {
		await renderEncoder((defaults) => {
			defaults.facts.bitrateKbps = 193;
		});
		await vi.waitFor(() => expect(screen.queryByTestId('native-speed-select')).not.toBeNull());

		changeSelectValue(screen.getByTestId<HTMLSelectElement>('native-speed-select'), '4');
		const bitrate = screen.getByTestId<HTMLInputElement>('bitrate-input');
		bitrate.value = '0';
		bitrate.dispatchEvent(new Event('change', { bubbles: true }));

		await vi.waitFor(() =>
			expect(sent()).toEqual([
				{ kind: 'setDefaultAudio', edit: { field: 'nativeSpeed', value: 4 } },
				{ kind: 'setDefaultAudio', edit: { field: 'bitrate', value: 0 } },
			]),
		);
		engine.change(() => undefined);
		await vi.waitFor(() => expect(bitrate.value).toBe('193'));
	});

	it('keeps the Mixed bitrate blank after an edit the engine refuses', async () => {
		engine = createFakeEngine();
		engine.change((state) => {
			state.audio.defaults.choice.intent = 'encode';
			state.audio.selection = {
				choice: state.audio.defaults.choice,
				facts: { ...state.audio.defaults.facts, bitrateKbps: 193 },
				mixed: ['bitrate'],
				autoSampleRate: { outcome: { kind: 'unresolved' }, problem: null },
				autoChannels: { outcome: { kind: 'unresolved' }, problem: null },
			};
		});
		runtime = await createAppRuntime({ engine });
		const files = [audioFile('/books/a.m4b', { status: 'ready' })];
		render(() => (
			<AppRuntimeProvider runtime={runtime!}>
				<EncoderView scope={{ kind: 'selection', files }} />
			</AppRuntimeProvider>
		));
		const bitrate = await screen.findByTestId<HTMLInputElement>('bitrate-input');
		expect(bitrate.value).toBe('');

		bitrate.value = '0';
		bitrate.dispatchEvent(new Event('change', { bubbles: true }));

		expect(bitrate.value).toBe('');
	});

	it('shows the engine encoder after a pick the engine did not take', async () => {
		await renderEncoder((defaults) => {
			defaults.facts.encoderOptions = defaults.facts.encoderOptions.map((option) => ({
				...option,
				available: true,
			}));
		});
		await vi.waitFor(() => expect(screen.queryByTestId('encoder-select')).not.toBeNull());
		const select = screen.getByTestId<HTMLSelectElement>('encoder-select');
		const held = select.value;
		const other = Array.from(select.options).find(
			(option) => option.value !== held && !option.disabled,
		);

		changeSelectValue(select, other!.value);

		await vi.waitFor(() => expect(sent()).toHaveLength(1));
		expect(select.value).toBe(held);
	});

	it('renders option ranges from the engine facts and capabilities', async () => {
		await renderEncoder((defaults, audio) => {
			defaults.facts.bitrateKbpsMin = 24;
			defaults.facts.bitrateKbpsMax = 256;
			defaults.facts.channelOptions = ['auto', 'mono'];
			if (audio.capabilities) audio.capabilities.explicitSampleRates = [44100];
		});

		await vi.waitFor(() => {
			const bitrate = screen.getByTestId<HTMLInputElement>('bitrate-input');
			const values = (id: string) =>
				Array.from(screen.getByTestId<HTMLSelectElement>(id).options).map((option) => option.value);
			expect(bitrate.min).toBe('24');
			expect(bitrate.max).toBe('256');
			expect(values('samplerate-select')).toEqual(['auto', '44100']);
			expect(values('channels-select')).toEqual(['auto', 'mono']);
		});
	});

	it('keeps default source hints independent of the selected title', async () => {
		await renderEncoder(() => undefined);
		engine.loadTitles(
			[
				audioFile('/books/source.m4b', {
					status: 'ready',
					label: 'source.m4b',
					sampleRate: 44100,
					channels: 2,
				}),
			],
			[0],
		);

		await vi.waitFor(() => {
			const shown = (id: string) =>
				screen.getByTestId<HTMLSelectElement>(id).selectedOptions[0]?.textContent;
			expect(shown('samplerate-select')).toBe('Auto · Source audio');
			expect(shown('channels-select')).toBe('Auto · Source audio');
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
			};
		});

		await vi.waitFor(() => expect(screen.queryByTestId('quality-select')?.hidden).toBe(false));
		const quality = screen.getByTestId<HTMLSelectElement>('quality-select');
		expect(Array.from(quality.options).map((option) => option.textContent)).toEqual([
			'Smaller (50)',
			'Standard (100)',
			'Higher (200)',
		]);
		const profile = screen.getByTestId<HTMLSelectElement>('faac-profile-select');
		expect(profile.selectedOptions[0]?.textContent).toBe('Auto · FAAC chooses LC or HE');
		const sampleRate = screen.getByTestId<HTMLSelectElement>('samplerate-select');
		expect(sampleRate.value).toBe('22050');
		expect(
			Array.from(sampleRate.options).find((option) => option.value === '22050')?.disabled,
		).toBe(true);

		changeSelectValue(quality, '50');
		changeSelectValue(screen.getByTestId<HTMLSelectElement>('faac-rate-control-select'), 'abr');
		await vi.waitFor(() =>
			expect(sent()).toEqual([
				{ kind: 'setDefaultAudio', edit: { field: 'quality', value: 50 } },
				{
					kind: 'setDefaultAudio',
					edit: { field: 'rateControl', value: 'abr' },
				},
			]),
		);
	});

	it('keeps showing a chosen encoder that is no longer available', async () => {
		await renderEncoder((defaults) => {
			defaults.choice.encoder = 'aac_at';
			defaults.facts.effectiveEncoder = 'aac_at';
			defaults.facts.encoderOptions = defaults.facts.encoderOptions.map((option) =>
				option.encoder === 'aac_at' ? { ...option, available: false } : option,
			);
		});

		await vi.waitFor(() => {
			const select = screen.getByTestId<HTMLSelectElement>('encoder-select');
			expect(select.value).toBe('aac_at');
			expect(Array.from(select.options).find((option) => option.value === 'aac_at')?.disabled).toBe(
				true,
			);
		});
	});
});
