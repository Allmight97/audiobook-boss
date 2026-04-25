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
} from '../outputPanel/state.svelte';

vi.mock('../../lib/tauri/client', () => ({
	tauriClient: {
		previewOutputPath: vi.fn(),
	},
}));

describe('output panel preview resilience', () => {
	beforeEach(() => {
		vi.mocked(tauriClient.previewOutputPath).mockReset();
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
		vi.mocked(tauriClient.previewOutputPath).mockResolvedValue('/Library/Audiobooks/Ghosts.m4b');
	});

	afterEach(() => {
		vi.clearAllMocks();
	});

	it('renders the backend preview path when output configuration is valid', async () => {
		updateOutputPath();

		const previewText = document.getElementById('output-preview-text');
		await waitFor(() => {
			expect(vi.mocked(tauriClient.previewOutputPath)).toHaveBeenCalledTimes(1);
			expect(outputPanelState.previewText).toContain('/Library/Audiobooks');
			expect(previewText?.textContent).toContain('/Library/Audiobooks');
		});
	});

	it('shows explicit preview error when Tauri preview RPC fails', async () => {
		vi.mocked(tauriClient.previewOutputPath).mockRejectedValueOnce(new Error('rpc down'));

		updateOutputPath();
		await new Promise((resolve) => setTimeout(resolve, 0));

		const previewText = document.getElementById('output-preview-text');
		expect(vi.mocked(tauriClient.previewOutputPath).mock.calls.length).toBeGreaterThan(0);
		expect(outputPanelState.previewText).toBe(
			'Output preview unavailable. Fix metadata/template and retry.',
		);
		expect(previewText?.textContent).toBe(
			'Output preview unavailable. Fix metadata/template and retry.',
		);
		expect(previewText?.textContent).not.toContain('/Library/Audiobooks');
	});

	it('requests preview artifact naming when asked to show a preview destination', async () => {
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
		vi.mocked(tauriClient.previewOutputPath).mockResolvedValueOnce(
			'/Library/Audiobooks/Frank Herbert/Dune.m4b',
		);

		updateOutputPath();

		await waitFor(() => {
			expect(outputPanelState.previewText).toContain('/Library/Audiobooks/Frank Herbert');
			expect(outputPanelState.previewText).toContain('Dune');
		});
		expect(outputPanelState.previewText).not.toContain('Unknown Author');
		expect(metadataFormState.seriesPartWarning.visible).toBe(true);
	});
});
