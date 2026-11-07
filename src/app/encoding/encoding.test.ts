import { afterEach, describe, expect, it, vi } from 'vitest';
import { audioFile, createFakeEngine, type FakeEngine } from '../../test/fixtures/fakeEngine';
import { createAppRuntime, type AppRuntime } from '../runtime';
import type { AutoReading, SelectionAudio } from '../../lib/generated/tauri';
import type { EncodingView } from './project';

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
		runtime = await createAppRuntime({ engine });
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
				{
					kind: 'setDefaultAudio',
					edit: { field: 'sampleRate', value: { explicit: 48000 } },
				},
				{
					kind: 'setDefaultAudio',
					edit: { field: 'sampleRate', value: 'auto' },
				},
				{
					kind: 'setDefaultAudio',
					edit: { field: 'rateControl', value: 'vbr' },
				},
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

	it("words Auto from each title's engine reading, never from source facts", async () => {
		const app = await open();
		const [alpha, beta] = titles(app);
		const autoLabel = (view: EncodingView, field: 'sampleRateOptions' | 'channelOptions') =>
			view[field].find((option) => option.value === 'auto')?.label;
		const unresolved = { outcome: { kind: 'unresolved' }, problem: null } as const;
		engine.change((state) => {
			const readings = (id: string, autoSampleRate: AutoReading, autoChannels: AutoReading) => {
				state.audio.titles[id] = {
					...structuredClone(state.audio.titles[id]),
					autoSampleRate,
					autoChannels,
				};
			};
			readings(
				alpha.inputId,
				{ outcome: { kind: 'value', value: 48000 }, problem: null },
				{ outcome: { kind: 'value', value: 2 }, problem: null },
			);
			readings(beta.inputId, unresolved, {
				outcome: { kind: 'unresolved' },
				problem: 'Choose Mono or Stereo to downmix.',
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
		expect(autoLabel(app.encoding.view(), 'sampleRateOptions')).toBe('Auto · Source audio');
	});

	it('shows the selection the engine combined, blanking the fields it names as mixed', async () => {
		const app = await open();
		const [alpha] = titles(app);
		engine.change((state) => {
			const first = state.audio.titles[alpha.inputId];
			state.audio.selection = {
				choice: first.choice,
				facts: { ...first.facts, downmixWarning: true },
				mixed: ['channels'],
				autoSampleRate: { outcome: { kind: 'unresolved' }, problem: null },
				autoChannels: { outcome: { kind: 'unresolved' }, problem: null },
			};
		});

		const view = app.encoding.selectionView();
		expect(view?.mixedFields).toEqual(['channels']);
		expect(view?.channelsHint).toBe('Surround downmix omits bass effects (LFE).');
	});
	it("words a selection's Auto from the engine's reading of its titles", async () => {
		const app = await open();
		const [alpha] = titles(app);
		const autoLabel = (view: EncodingView | null, field: 'sampleRateOptions' | 'channelOptions') =>
			view?.[field].find((option) => option.value === 'auto')?.label;
		const select = (
			autoSampleRate: SelectionAudio['autoSampleRate'],
			autoChannels: SelectionAudio['autoChannels'],
		) =>
			engine.change((state) => {
				const first = state.audio.titles[alpha.inputId];
				state.audio.selection = {
					choice: first.choice,
					facts: first.facts,
					mixed: [],
					autoSampleRate,
					autoChannels,
				};
			});

		select(
			{ outcome: { kind: 'value', value: 48000 }, problem: null },
			{
				outcome: { kind: 'differs' },
				problem: 'Choose Mono or Stereo to downmix.',
			},
		);
		const view = app.encoding.selectionView();
		expect(autoLabel(view, 'sampleRateOptions')).toBe('Auto · 48 kHz');
		expect(autoLabel(view, 'channelOptions')).toBe('Auto · Differs by title');
		expect(view?.channelsHint).toBe('Choose Mono or Stereo to downmix.');

		select(
			{ outcome: { kind: 'unresolved' }, problem: null },
			{ outcome: { kind: 'unresolved' }, problem: null },
		);
		expect(autoLabel(app.encoding.selectionView(), 'sampleRateOptions')).toBe('Auto');
	});
	it('names the titles that refused an edit, and says when the list is locked', async () => {
		const app = await open();
		const [alpha, beta] = titles(app);
		engine.change((state) => {
			state.audio.refusal = {
				kind: 'notAccepted',
				titleIds: [beta.inputId],
				labels: ['Beta Book'],
			};
		});
		expect(app.encoding.refusal([alpha, beta])).toBe(
			"Beta Book can't take that change, so no title changed.",
		);
		expect(app.encoding.refusal([alpha])).toBeNull();

		engine.change((state) => {
			state.audio.refusal = { kind: 'locked' };
		});
		expect(app.encoding.refusal([alpha])).toBe(
			"Audio can't change while an export is being prepared.",
		);
	});
	it('renders the engine downmix warning even when the anchor alone is stereo', async () => {
		const app = await open();
		const alpha = titles(app)[0];
		engine.change((state) => {
			state.audio.titles[alpha.inputId].facts.downmixWarning = true;
		});
		expect(app.encoding.titleView(alpha).channelsHint).toBe(
			'Surround downmix omits bass effects (LFE).',
		);
		engine.change((state) => {
			state.audio.titles[alpha.inputId].facts.downmixWarning = false;
		});
		expect(app.encoding.titleView(alpha).channelsHint).toBeNull();
	});
});
