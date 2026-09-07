import { describe, expect, it } from 'vitest';
import type { AudioFile } from '../../types/audio';
import { resolveAutoResolutionHints } from './hints';

const makeFile = (overrides: Partial<AudioFile>): AudioFile => ({
	path: '/books/input.m4b',
	isValid: true,
	...overrides,
});

describe('resolveAutoResolutionHints', () => {
	it('handles single exact values', () => {
		const hints = resolveAutoResolutionHints([
			makeFile({
				sampleRate: 44100,
				channels: 2,
			}),
		]);

		expect(hints.sampleRateHint).toBe('Auto -> 44.1 kHz');
		expect(hints.channelsHint).toBe('Auto -> Stereo');
	});

	it('handles multi-selection when all selected files share the same values', () => {
		const hints = resolveAutoResolutionHints([
			makeFile({ path: '/books/one.m4b', sampleRate: 32000, channels: 1 }),
			makeFile({ path: '/books/two.m4b', sampleRate: 32000, channels: 1 }),
			makeFile({ path: '/books/three.m4b', sampleRate: 32000, channels: 1 }),
		]);

		expect(hints.sampleRateHint).toBe('Auto -> 32 kHz');
		expect(hints.channelsHint).toBe('Auto -> Mono');
	});

	it('handles multi-selection with mixed input values', () => {
		const hints = resolveAutoResolutionHints([
			makeFile({ path: '/books/one.m4b', sampleRate: 32000, channels: 1 }),
			makeFile({ path: '/books/two.m4b', sampleRate: 44100, channels: 2 }),
		]);

		expect(hints.sampleRateHint).toBe('Auto -> mixed (32/44.1 kHz)');
		expect(hints.channelsHint).toBe('Auto -> source channels; Stereo when merged');
		expect(
			resolveAutoResolutionHints([makeFile({ channels: 2 }), makeFile({ channels: 1 })])
				.channelsHint,
		).toBe('Auto -> source channels; Stereo when merged');
	});

	it('treats partial metadata in multi-selection as mixed inputs', () => {
		const hints = resolveAutoResolutionHints([
			makeFile({ path: '/books/known.m4b', sampleRate: 44100, channels: 2 }),
			makeFile({ path: '/books/unknown.m4b' }),
		]);

		expect(hints.sampleRateHint).toBe('Auto -> mixed/unknown rates');
		expect(hints.channelsHint).toBe('Unknown input channels: choose Mono or Stereo.');
	});

	it('falls back to unknown helper text when values are missing', () => {
		expect(resolveAutoResolutionHints([])).toEqual({
			sampleRateHint: 'Auto -> source audio',
			channelsHint: 'Auto -> source audio',
			hasMultichannelInput: false,
		});

		expect(
			resolveAutoResolutionHints([
				makeFile({
					sampleRate: undefined,
					channels: undefined,
				}),
			]),
		).toEqual({
			sampleRateHint: 'Auto -> source audio',
			channelsHint: 'Unknown input channels: choose Mono or Stereo.',
			hasMultichannelInput: false,
		});
	});

	it('formats supported audiobook sample rates compactly', () => {
		expect(
			resolveAutoResolutionHints([
				makeFile({ path: '/books/a.m4b', sampleRate: 22050, channels: 1 }),
				makeFile({ path: '/books/b.m4b', sampleRate: 48000, channels: 2 }),
			]).sampleRateHint,
		).toBe('Auto -> mixed (22.05/48 kHz)');
	});
	it('requires an explicit downmix for multichannel input', () => {
		expect(resolveAutoResolutionHints([makeFile({ channels: 6 })]).channelsHint).toBe(
			'Multichannel input: choose Mono or Stereo to downmix.',
		);
	});
});
