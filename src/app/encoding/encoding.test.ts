import { afterEach, describe, expect, it, vi } from 'vitest';
import { audioFile, createFakeEngine, type FakeEngine } from '../../test/fixtures/fakeEngine';
import { createAppRuntime, type AppRuntime } from '../runtime';
import type { TitlePlan } from '../../types/session';
import type { EncodingView } from './project';

// The audio choice rules live in the engine (`crates/abb-engine/src/session/
// audio_choice.rs`). These tests cover what this owner adds: the panel's
// terms for the engine's choice, and the edit each control sends.

describe('encoding owner', () => {
	let runtime: AppRuntime | undefined;
	let engine: FakeEngine;

	afterEach(() => {
		runtime?.dispose();
		runtime = undefined;
	});

	async function open(): Promise<AppRuntime> {
		engine = createFakeEngine();
		engine.loadTitles([
			audioFile('/books/alpha.m4b', { sampleRate: 44100, channels: 2 }),
			audioFile('/books/beta.m4b', { sampleRate: 22050, channels: 1 }),
		]);
		runtime = createAppRuntime({ engine });
		await runtime.initialize();
		return runtime;
	}

	function titles(app: AppRuntime) {
		return app.input.view().files;
	}

	it('sends each control as a typed edit and drops values no control offers', async () => {
		const app = await open();

		app.encoding.select('bitrate', '96');
		app.encoding.select('sampleRate', '48000');
		app.encoding.select('sampleRate', 'auto');
		app.encoding.select('rateControl', 'vbr');
		app.encoding.select('bitrate', 'fast');
		app.encoding.select('format', 'wav');

		await vi.waitFor(() =>
			expect(engine.sessionIntents.filter((intent) => intent.kind === 'setDefaultAudio')).toEqual([
				{ kind: 'setDefaultAudio', edit: { field: 'bitrate', value: 96 } },
				{ kind: 'setDefaultAudio', edit: { field: 'sampleRate', value: { explicit: 48000 } } },
				{ kind: 'setDefaultAudio', edit: { field: 'sampleRate', value: 'auto' } },
				{ kind: 'setDefaultAudio', edit: { field: 'rateControl', value: 'vbr' } },
			]),
		);
	});

	it('edits titles by identity and copies the defaults onto them', async () => {
		const app = await open();
		const [alpha, beta] = titles(app);

		app.encoding.selectTitles([alpha, beta], 'channels', 'mono');
		app.encoding.applyDefaultsToTitles([beta]);

		await vi.waitFor(() =>
			expect(engine.sessionIntents.slice(-2)).toEqual([
				{
					kind: 'setTitleAudio',
					titleIds: [alpha.inputId, beta.inputId],
					edit: { field: 'channels', value: 'mono' },
				},
				{ kind: 'applyDefaultAudio', titleIds: [beta.inputId] },
			]),
		);
	});

	it('shows what the engine allows and why a kept rate needs replacing', async () => {
		const app = await open();
		engine.change((state) => {
			state.audio.defaults.choice.sampleRate = { explicit: 22050 };
			state.audio.defaults.facts.allowedSampleRates = [44100, 48000];
			state.audio.defaults.facts.sampleRateSupported = false;
		});

		const view = app.encoding.view();
		expect(view.sampleRate).toBe('22050');
		expect(view.sampleRateHint).toBe('Choose a supported sample rate for this encoder.');
		const options = Object.fromEntries(
			view.sampleRateOptions.map((option) => [option.value, Boolean(option.disabled)]),
		);
		expect(options['22050']).toBe(true);
		expect(options['48000']).toBe(false);
		expect(options.auto).toBe(false);
	});

	it("words Auto from each title's engine plan, never from source facts", async () => {
		const app = await open();
		const [alpha, beta] = titles(app);
		const autoLabel = (view: EncodingView, field: 'sampleRateOptions' | 'channelOptions') =>
			view[field].find((option) => option.value === 'auto')?.label;
		engine.change((state) => {
			const plan = (id: string, value: TitlePlan) => {
				state.audio.titles[id] = { ...structuredClone(state.audio.titles[id]), plan: value };
			};
			// The engine raised alpha's 44.1 kHz source to what its encoder accepts.
			plan(alpha.inputId ?? '', {
				kind: 'resolved',
				plan: {
					format: 'm4b',
					handling: 'encode',
					settings: null,
					sampleRate: 48000,
					channels: 2,
					sourceCodec: 'AAC',
					reason: null,
				},
			});
			plan(beta.inputId ?? '', {
				kind: 'failed',
				message: 'Choose Mono or Stereo to downmix.',
				field: 'channels',
			});
		});

		const alphaView = app.encoding.titleView(alpha);
		expect(autoLabel(alphaView, 'sampleRateOptions')).toBe('Auto · 48 kHz');
		expect(autoLabel(alphaView, 'channelOptions')).toBe('Auto · Stereo');
		expect(alphaView.channelsHint).toBeNull();
		const betaView = app.encoding.titleView(beta);
		expect(autoLabel(betaView, 'channelOptions')).toBe('Auto');
		expect(betaView.channelsHint).toBe('Choose Mono or Stereo to downmix.');
		expect(betaView.sampleRateHint).toBeNull();
		// The defaults describe future imports, so they name no source.
		expect(autoLabel(app.encoding.view(), 'sampleRateOptions')).toBe('Auto · Source audio');
	});

	it('marks fields that differ across selected titles', async () => {
		const app = await open();
		const [alpha, beta] = titles(app);
		engine.change((state) => {
			const id = beta.inputId ?? '';
			state.audio.titles[id] = structuredClone(state.audio.titles[id]);
			state.audio.titles[id].choice.channels = 'mono';
		});

		expect(app.encoding.titleView(beta).channels).toBe('mono');
		expect(app.encoding.selectionView([alpha, beta]).mixedFields).toEqual(['channels']);
	});
	it('renders the engine downmix warning even when the anchor alone is stereo', async () => {
		const app = await open();
		const alpha = titles(app)[0]!;
		engine.change((state) => {
			state.audio.titles[alpha.inputId!]!.facts.downmixWarning = true;
		});
		expect(app.encoding.titleView(alpha).channelsHint).toBe(
			'Surround downmix omits bass effects (LFE).',
		);
		engine.change((state) => {
			state.audio.titles[alpha.inputId!]!.facts.downmixWarning = false;
		});
		expect(app.encoding.titleView(alpha).channelsHint).toBeNull();
	});
});
