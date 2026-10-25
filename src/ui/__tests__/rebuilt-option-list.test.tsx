import { flush } from 'solid-js';
import { cleanup, render } from '@solidjs/testing-library';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { AppRuntimeProvider, createAppRuntime, type AppRuntime } from '../../app/runtime';
import { createFakeEngine, fakeEngine, type FakeEngine } from '../../test/fixtures/fakeEngine';
import type { AudioChoiceView, SessionAudio } from '../../types/session';
import { EncoderView } from '../encoderPanel';
import { RemoteSourceAcquireView } from '../remoteSource';

// Keyed option lists keep the chosen value when the engine publishes a new
// option array. An unkeyed list follows position, so a rebuilt list would
// show a different choice.

type Row = {
	readonly name: string;
	readonly selectId: string;
	readonly expected: string;
	mount(): Promise<void>;
	rebuild(): void;
	prove(select: HTMLSelectElement): void;
};

describe('rebuilt option lists keep the selection', () => {
	let runtime: AppRuntime | undefined;
	let engine: FakeEngine;

	afterEach(() => {
		cleanup();
		runtime?.dispose();
		runtime = undefined;
		document.body.innerHTML = '';
	});

	async function mountEncoder(seed: (defaults: AudioChoiceView, audio: SessionAudio) => void) {
		engine = createFakeEngine();
		engine.change((state) => {
			state.audio.defaults.choice.intent = 'encode';
			seed(state.audio.defaults, state.audio);
		});
		runtime = createAppRuntime({ engine });
		await runtime.initialize();
		render(() => (
			<AppRuntimeProvider runtime={runtime!}>
				<EncoderView />
			</AppRuntimeProvider>
		));
	}

	function faac(defaults: AudioChoiceView): void {
		defaults.choice.encoder = 'faac';
		defaults.choice.faacRateControl = 'vbr';
		defaults.choice.faacProfile = 'aac_lc';
		defaults.choice.faacQuality = 200;
		defaults.facts = {
			...defaults.facts,
			effectiveEncoder: 'faac',
			bitrateMode: { mode: 'vbr', value: 200 },
			allowedModes: ['abr', 'vbr'],
			faacProfiles: ['auto', 'aac_lc', 'he_aac_v1'],
		};
	}

	const rows: readonly Row[] = [
		{
			name: 'native speed',
			selectId: 'native-speed',
			expected: '2',
			async mount() {
				await mountEncoder((defaults, audio) => {
					defaults.choice.nativeSpeed = 2;
					defaults.choice.channels = 'stereo';
					if (audio.capabilities) audio.capabilities.nativeSpeedMax = 4;
				});
			},
			rebuild() {
				engine.change((state) => {
					if (state.audio.capabilities) state.audio.capabilities.nativeSpeedMax = 5;
				});
			},
			prove(select) {
				expect(Array.from(select.options).map((option) => option.value)).toContain('5');
			},
		},
		{
			name: 'FAAC profile',
			selectId: 'faac-profile',
			expected: 'aac_lc',
			async mount() {
				await mountEncoder((defaults) => {
					faac(defaults);
				});
			},
			rebuild() {
				engine.change((state) => {
					state.audio.defaults.facts.faacProfiles = ['he_aac_v1', 'auto', 'aac_lc'];
				});
			},
			prove(select) {
				expect(select.options[0]?.value).toBe('he_aac_v1');
			},
		},
		{
			name: 'rate control',
			selectId: 'faac-rate-control',
			expected: 'abr',
			async mount() {
				await mountEncoder((defaults) => {
					faac(defaults);
					defaults.choice.faacRateControl = 'abr';
				});
			},
			rebuild() {
				engine.change((state) => {
					state.audio.defaults.facts.allowedModes = ['vbr', 'abr'];
				});
			},
			prove(select) {
				expect(select.options[0]?.value).toBe('vbr');
			},
		},
		{
			name: 'quality',
			selectId: 'output-quality',
			expected: '200',
			async mount() {
				await mountEncoder((defaults) => {
					faac(defaults);
				});
			},
			rebuild() {
				engine.change((state) => {
					if (state.audio.capabilities) {
						state.audio.capabilities.faacQualityPresets = [50, 200, 100];
					}
				});
			},
			prove(select) {
				expect(Array.from(select.options).map((option) => option.value)).toEqual([
					'50',
					'200',
					'100',
				]);
			},
		},
		{
			name: 'channels',
			selectId: 'output-channels',
			expected: 'stereo',
			async mount() {
				await mountEncoder((defaults, audio) => {
					defaults.choice.channels = 'stereo';
					if (audio.capabilities) audio.capabilities.channelOptions = ['auto', 'mono', 'stereo'];
				});
			},
			rebuild() {
				engine.change((state) => {
					if (state.audio.capabilities) {
						state.audio.capabilities.channelOptions = ['auto', 'stereo', 'mono'];
					}
				});
			},
			prove(select) {
				expect(select.options[1]?.value).toBe('stereo');
			},
		},
		{
			name: 'provider picker',
			selectId: 'remote-source-provider',
			expected: 'audible',
			async mount() {
				runtime = createAppRuntime();
				await runtime.initialize();
				render(() => (
					<AppRuntimeProvider runtime={runtime!}>
						<RemoteSourceAcquireView />
					</AppRuntimeProvider>
				));
				fakeEngine().change((state) => {
					state.remote.lane = 'audible';
					state.remote.providers = [
						{ providerId: 'audible', label: 'Audible' },
						{ providerId: 'indexer', label: 'Indexer' },
					];
					state.remote.account = { providerId: 'audible', status: 'connected' };
				});
				await runtime.remoteSource.open({ lane: 'audible' });
			},
			rebuild() {
				fakeEngine().change((state) => {
					state.remote.providers = [
						{ providerId: 'indexer', label: 'Indexer' },
						{ providerId: 'audible', label: 'Audible' },
					];
				});
			},
			prove(select) {
				expect(select.options[0]?.value).toBe('indexer');
			},
		},
	];

	it.each(rows)('$name', async (row) => {
		await row.mount();
		const select = () => document.getElementById(row.selectId) as HTMLSelectElement | null;
		await vi.waitFor(() => expect(select()?.value).toBe(row.expected));
		row.rebuild();
		flush();
		// Solid writes a select's value in a microtask, after the option nodes move.
		await vi.waitFor(() => expect(select()?.value).toBe(row.expected));
		const rebuilt = select();
		if (rebuilt) row.prove(rebuilt);
	});
});
