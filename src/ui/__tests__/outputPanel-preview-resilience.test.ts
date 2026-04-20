import { render, waitFor } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { tauriClient } from '../../lib/tauri/client';
import OutputPanelIsland from '../outputPanel/OutputPanelIsland.svelte';
import { setJobTypeSelection } from '../jobControls';
import { populateMetadataFormMulti, populateMetadataFormSingle } from '../metadataForm';
import { metadataFormState } from '../metadataForm/state.svelte';
import { updateOutputPath } from '../outputPanel/dom';
import {
	outputPanelState,
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
		render(OutputPanelIsland);
		populateMetadataFormSingle({
			title: 'Ghosts',
			album: 'Ghosts',
			artist: 'Ryk Brown',
		});
		updateOutputDirectory('/Library/Audiobooks');
		updateNamingPreset('absDefault');
		updateAbsIncludeYear(false);
		setJobTypeSelection('merge');
	});

	afterEach(() => {
		(globalThis as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = undefined;
		vi.clearAllMocks();
	});

	it('falls back to local preview path when Tauri runtime is unavailable', async () => {
		updateOutputPath();

		const previewText = document.getElementById('output-preview-text');
		expect(outputPanelState.previewText).toContain('/Library/Audiobooks');
		await waitFor(() => {
			expect(previewText?.textContent).toContain('/Library/Audiobooks');
		});
		expect(vi.mocked(tauriClient.previewOutputPath)).not.toHaveBeenCalled();
	});

	it('shows explicit preview error when Tauri preview RPC fails', async () => {
		(globalThis as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {};
		vi.mocked(tauriClient.previewOutputPath).mockRejectedValueOnce(new Error('rpc down'));

		updateOutputPath();
		await new Promise((resolve) => setTimeout(resolve, 0));

		const previewText = document.getElementById('output-preview-text');
		expect(vi.mocked(tauriClient.previewOutputPath)).toHaveBeenCalledTimes(1);
		expect(outputPanelState.previewText).toBe(
			'Output preview unavailable. Fix metadata/template and retry.',
		);
		expect(previewText?.textContent).toBe(
			'Output preview unavailable. Fix metadata/template and retry.',
		);
		expect(previewText?.textContent).not.toContain('/Library/Audiobooks');
	});

	it('requests preview artifact naming when asked to show a preview destination', async () => {
		(globalThis as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {};
		vi.mocked(tauriClient.previewOutputPath).mockResolvedValueOnce(
			'/Library/Audiobooks/Ghosts.preview.m4b',
		);

		updateOutputPath('preview');
		await new Promise((resolve) => setTimeout(resolve, 0));

		expect(vi.mocked(tauriClient.previewOutputPath)).toHaveBeenCalledWith(
			expect.objectContaining({
				outputKind: 'preview',
			}),
		);
		expect(outputPanelState.previewText).toBe('/Library/Audiobooks/Ghosts.preview.m4b');
	});

	it('clears hidden output directory mirror when directory state is emptied', async () => {
		const hiddenDirInput = document.getElementById('output-dir-text') as HTMLInputElement;
		updateOutputDirectory('');

		updateOutputPath();

		expect(outputPanelState.outputDirectory).toBe('');
		await waitFor(() => {
			expect(hiddenDirInput.value).toBe('');
		});
	});

	it('uses shared multi-select metadata for preview text and series warnings', async () => {
		populateMetadataFormMulti(
			[
				{ title: 'Dune', artist: 'Frank Herbert', series: 'Dune' },
				{ title: 'Dune', artist: 'Frank Herbert', series: 'Dune' },
			],
			2,
		);

		updateOutputPath();

		await waitFor(() => {
			expect(outputPanelState.previewText).toContain('/Library/Audiobooks/Frank Herbert/');
			expect(outputPanelState.previewText).toContain('Dune');
		});
		expect(outputPanelState.previewText).not.toContain('Unknown Author');
		expect(metadataFormState.seriesPartWarning.visible).toBe(true);
	});
});
