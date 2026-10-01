import { afterEach, describe, expect, it, vi } from 'vitest';
import { audioFile, createFakeEngine, type FakeEngine } from '../../test/fixtures/fakeEngine';
import { createAppRuntime, type AppRuntime } from '../runtime';

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

	it('words a title from its own choice and its sources, and marks mixed fields', async () => {
		const app = await open();
		const [alpha, beta] = titles(app);
		engine.change((state) => {
			const id = beta.inputId ?? '';
			state.audio.titles[id] = structuredClone(state.audio.titles[id]);
			state.audio.titles[id].choice.channels = 'mono';
		});

		expect(app.encoding.titleView(alpha).sampleRateHint).toBe('Auto -> 44.1 kHz');
		expect(app.encoding.titleView(beta).channels).toBe('mono');
		expect(app.encoding.selectionView([alpha, beta]).mixedFields).toEqual(['channels']);
		// The defaults describe future imports, so they name no source.
		expect(app.encoding.view().sampleRateHint).toBe('Auto -> source audio');
	});
});
