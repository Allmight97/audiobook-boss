/**
 * Unit coverage for the cover-art inline-message lifecycle.
 *
 * Pairs with the `CoverArtMessage` discriminated union refactor in
 * `state.svelte.ts`. Exercises `showCoverArtMessage` / `clearCoverArtMessage`
 * from `../../coverArt` directly because the 4-second auto-dismiss timer
 * handle lives in module scope there.
 *
 * Module-scope state forces per-case isolation: each test uses
 * `vi.resetModules()` + fresh dynamic import, otherwise the module-scoped
 * `coverArtMessageTimeoutId` leaks between cases.
 */

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

type CoverArtModule = typeof import('../../coverArt');
type CoverArtStateModule = typeof import('../state.svelte');

async function loadFreshModules(): Promise<{
	coverArt: CoverArtModule;
	state: CoverArtStateModule;
}> {
	vi.resetModules();
	const coverArt = await import('../../coverArt');
	const state = await import('../state.svelte');
	return { coverArt, state };
}

describe('CoverArtMessage lifecycle', () => {
	beforeEach(() => {
		vi.useFakeTimers();
	});

	afterEach(() => {
		vi.useRealTimers();
	});

	it('sets the success variant when showCoverArtMessage is called with success', async () => {
		const { coverArt, state } = await loadFreshModules();

		coverArt.showCoverArtMessage('Cover art loaded from URL.', 'success');

		expect(state.coverArtUiState.message).toEqual({
			kind: 'success',
			text: 'Cover art loaded from URL.',
		});
	});

	it('sets the error variant when showCoverArtMessage is called with error', async () => {
		const { coverArt, state } = await loadFreshModules();

		coverArt.showCoverArtMessage('Invalid URL format.', 'error');

		expect(state.coverArtUiState.message).toEqual({
			kind: 'error',
			text: 'Invalid URL format.',
		});
	});

	it('auto-dismisses to kind: hidden after 4 seconds', async () => {
		const { coverArt, state } = await loadFreshModules();

		coverArt.showCoverArtMessage('Invalid URL format.', 'error');
		expect(state.coverArtUiState.message.kind).toBe('error');

		vi.advanceTimersByTime(4000);

		expect(state.coverArtUiState.message).toEqual({ kind: 'hidden' });
	});

	it('does not auto-dismiss before 4 seconds', async () => {
		const { coverArt, state } = await loadFreshModules();

		coverArt.showCoverArtMessage('Invalid URL format.', 'error');

		vi.advanceTimersByTime(3999);

		expect(state.coverArtUiState.message.kind).toBe('error');
	});

	it('clearCoverArtMessage cancels the pending auto-dismiss so a subsequent show is not prematurely hidden', async () => {
		const { coverArt, state } = await loadFreshModules();

		coverArt.showCoverArtMessage('first message', 'error');
		vi.advanceTimersByTime(2000);

		coverArt.clearCoverArtMessage();
		expect(state.coverArtUiState.message).toEqual({ kind: 'hidden' });

		coverArt.showCoverArtMessage('second message', 'success');

		vi.advanceTimersByTime(2500);

		expect(state.coverArtUiState.message).toEqual({
			kind: 'success',
			text: 'second message',
		});
	});

	it('renders large embedded cover art without overflowing the JS call stack', async () => {
		const { coverArt, state } = await loadFreshModules();
		const bytes = new Array<number>(200_000).fill(0);
		bytes[0] = 0xff;
		bytes[1] = 0xd8;
		bytes[2] = 0xff;

		expect(() => coverArt.setCoverArt(bytes)).not.toThrow();
		expect(state.coverArtUiState.imageDataUrl?.startsWith('data:image/jpeg;base64,')).toBe(true);
	});
});
