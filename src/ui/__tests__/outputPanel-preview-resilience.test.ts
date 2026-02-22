import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { tauriClient } from '../../lib/tauri/client';
import { setJobTypeSelection } from '../jobControls';
import { updateOutputPath } from '../outputPanel/dom';
import {
	updateOutputDirectory,
	updateNamingPreset,
	updateAbsIncludeYear,
} from '../outputPanel/state';

vi.mock('../../lib/tauri/client', () => ({
	tauriClient: {
		previewOutputPath: vi.fn(),
	},
}));

describe('output panel preview resilience', () => {
	beforeEach(() => {
		(globalThis as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = undefined;
		document.body.innerHTML = `
      <div id="output-preview-text"></div>
      <input id="output-dir-text" />
      <input id="meta-title" value="Ghosts" />
      <input id="meta-author" value="Ryk Brown" />
      <input id="meta-narrator" value="" />
      <input id="meta-year" value="" />
      <input id="meta-genre" value="" />
      <textarea id="meta-description"></textarea>
      <input id="meta-series" value="" />
      <input id="meta-series-part" value="" />
      <input id="meta-subseries" value="" />
      <input id="meta-subseries-part" value="" />
      <div id="meta-series-part-warning" hidden></div>
      <div id="meta-subseries-part-warning" hidden></div>
    `;
		updateOutputDirectory('/Library/Audiobooks');
		updateNamingPreset('absDefault');
		updateAbsIncludeYear(false);
		setJobTypeSelection('merge');
	});

	afterEach(() => {
		(globalThis as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = undefined;
		vi.clearAllMocks();
	});

	it('falls back to local preview path when Tauri runtime is unavailable', () => {
		updateOutputPath();

		const previewText = document.getElementById('output-preview-text');
		expect(previewText?.textContent).toContain('/Library/Audiobooks');
		expect(vi.mocked(tauriClient.previewOutputPath)).not.toHaveBeenCalled();
	});

	it('shows explicit preview error when Tauri preview RPC fails', async () => {
		(globalThis as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {};
		vi.mocked(tauriClient.previewOutputPath).mockRejectedValueOnce(new Error('rpc down'));

		updateOutputPath();
		await new Promise((resolve) => setTimeout(resolve, 0));

		const previewText = document.getElementById('output-preview-text');
		expect(vi.mocked(tauriClient.previewOutputPath)).toHaveBeenCalledTimes(1);
		expect(previewText?.textContent).toBe(
			'Output preview unavailable. Fix metadata/template and retry.',
		);
		expect(previewText?.textContent).not.toContain('/Library/Audiobooks');
	});

	it('clears hidden output directory mirror when directory state is emptied', () => {
		const hiddenDirInput = document.getElementById('output-dir-text') as HTMLInputElement;
		hiddenDirInput.value = '/stale/path';
		updateOutputDirectory('');

		updateOutputPath();

		expect(hiddenDirInput.value).toBe('');
	});
});
