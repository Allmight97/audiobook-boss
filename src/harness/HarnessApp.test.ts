import { fireEvent, render, screen } from '@testing-library/svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const coverArtHandlers = vi.hoisted(() => ({
	onLoadFromFile: vi.fn(),
	onLoadFromInput: vi.fn(async (value: string) => value),
	onClearCoverArt: vi.fn(),
}));

vi.mock('../ui/coverArt', () => ({
	getCurrentCoverArt: vi.fn(() => null),
	getHasCustomCoverArt: vi.fn(() => false),
	isCoverArtRemovalRequested: vi.fn(() => false),
	setCoverArt: vi.fn(),
	onLoadCoverArtFromFilePicker: coverArtHandlers.onLoadFromFile,
	onLoadCoverArtFromInput: coverArtHandlers.onLoadFromInput,
	onClearCoverArt: coverArtHandlers.onClearCoverArt,
}));

import HarnessApp from '../HarnessApp.svelte';

describe('HarnessApp fixture-driven rendering', () => {
	beforeEach(() => {
		coverArtHandlers.onLoadFromFile.mockReset();
		coverArtHandlers.onLoadFromInput.mockReset();
		coverArtHandlers.onLoadFromInput.mockResolvedValue('https://example.com/cover.jpg');
		coverArtHandlers.onClearCoverArt.mockReset();
	});

	it('renders default harness fixtures for input and lookup panels', () => {
		render(HarnessApp);

		expect(screen.getByRole('heading', { name: 'Harness: Input' })).toBeInTheDocument();
		expect(
			screen.getByRole('heading', { name: 'Harness: Metadata Lookup Modal' }),
		).toBeInTheDocument();
		expect(screen.getByTestId('metadata-lookup-modal')).toBeInTheDocument();
	});

	it('honors fixture overrides for user-facing harness composition', () => {
		render(HarnessApp, {
			props: {
				fixture: {
					labels: {
						inputPanelTitle: 'Fixture Input Lane',
					},
					islands: {
						metadataLookup: { enabled: false },
					},
				},
			},
		});

		expect(screen.getByRole('heading', { name: 'Fixture Input Lane' })).toBeInTheDocument();
		expect(screen.queryByTestId('harness-metadata-lookup')).not.toBeInTheDocument();
		expect(screen.queryByTestId('metadata-lookup-modal')).not.toBeInTheDocument();
	});

	it('wires cover art actions through the production handler contract', async () => {
		render(HarnessApp);

		await fireEvent.click(screen.getByTestId('cover-art-area'));
		expect(coverArtHandlers.onLoadFromFile).toHaveBeenCalledTimes(1);

		const urlInput = screen.getByTestId('cover-art-url-input') as HTMLInputElement;
		await fireEvent.input(urlInput, { target: { value: 'https://example.com/cover.jpg' } });
		await fireEvent.click(screen.getByTestId('cover-art-url-load-btn'));
		expect(coverArtHandlers.onLoadFromInput).toHaveBeenCalledWith('https://example.com/cover.jpg');

		await fireEvent.click(document.getElementById('cover-art-clear-btn') as HTMLButtonElement);
		expect(coverArtHandlers.onClearCoverArt).toHaveBeenCalledTimes(1);
	});
});
