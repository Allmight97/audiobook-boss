import { describe, expect, it } from 'vitest';
import type { AudioFile } from '../../../types/audio';
import { channelCountToLabel, resolveAutoResolutionHints } from '../autoResolutionHints';

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

		expect(hints.sampleRateHint).toBe('Auto resolves to 44100 Hz from selected file.');
		expect(hints.channelsHint).toBe('Auto resolves to Stereo from selected file.');
	});

	it('handles multi-selection when all selected files share the same values', () => {
		const hints = resolveAutoResolutionHints([
			makeFile({ path: '/books/one.m4b', sampleRate: 32000, channels: 1 }),
			makeFile({ path: '/books/two.m4b', sampleRate: 32000, channels: 1 }),
			makeFile({ path: '/books/three.m4b', sampleRate: 32000, channels: 1 }),
		]);

		expect(hints.sampleRateHint).toBe('Auto resolves to 32000 Hz across selected files.');
		expect(hints.channelsHint).toBe('Auto resolves to Mono across selected files.');
	});

	it('handles multi-selection with mixed input values', () => {
		const hints = resolveAutoResolutionHints([
			makeFile({ path: '/books/one.m4b', sampleRate: 32000, channels: 1 }),
			makeFile({ path: '/books/two.m4b', sampleRate: 44100, channels: 2 }),
		]);

		expect(hints.sampleRateHint).toBe('Auto resolves per file (mixed inputs).');
		expect(hints.channelsHint).toBe('Auto resolves per file (mixed inputs).');
	});

	it('treats partial metadata in multi-selection as mixed inputs', () => {
		const hints = resolveAutoResolutionHints([
			makeFile({ path: '/books/known.m4b', sampleRate: 44100, channels: 2 }),
			makeFile({ path: '/books/unknown.m4b' }),
		]);

		expect(hints.sampleRateHint).toBe('Auto resolves per file (mixed inputs).');
		expect(hints.channelsHint).toBe('Auto resolves per file (mixed inputs).');
	});

	it('falls back to unknown helper text when values are missing', () => {
		expect(resolveAutoResolutionHints([])).toEqual({
			sampleRateHint: 'Auto resolves from source audio.',
			channelsHint: 'Auto resolves from source audio.',
		});

		expect(
			resolveAutoResolutionHints([
				makeFile({
					sampleRate: undefined,
					channels: undefined,
				}),
			]),
		).toEqual({
			sampleRateHint: 'Auto resolves from source audio.',
			channelsHint: 'Auto resolves from source audio.',
		});
	});
});

describe('channelCountToLabel', () => {
	it('maps channel counts to human-readable labels', () => {
		expect(channelCountToLabel(1)).toBe('Mono');
		expect(channelCountToLabel(2)).toBe('Stereo');
		expect(channelCountToLabel(6)).toBe('6 ch');
	});
});
