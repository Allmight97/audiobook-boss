import { render, screen } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';
import MetadataManagerIsland from '../metadataManager/MetadataManagerIsland.svelte';

vi.mock('../coverArt', () => ({
	onClearCoverArt: vi.fn(),
	onLoadCoverArtFromFilePicker: vi.fn(),
	onLoadCoverArtFromInput: vi.fn(),
}));

vi.mock('../metadataSession', () => ({
	saveMetadataFromUI: vi.fn(),
	metadataSaveInProgress: { subscribe: vi.fn(() => () => {}) },
}));

vi.mock('../metadataForm', () => ({
	onMetadataFormActionSelectChange: vi.fn(),
	onMetadataFormFieldInput: vi.fn(),
	initMetadataFormEvents: vi.fn(),
}));

vi.mock('../fileList', () => ({
	getCurrentFileList: vi.fn().mockReturnValue(null),
	getSelectedFileIndices: vi.fn().mockReturnValue(new Set()),
}));

vi.mock('../../lib/tauri/client', () => ({
	tauriClient: {},
}));

describe('MetadataManagerIsland composition', () => {
	it('arranges cover art, metadata form, and embedded artifacts zones', () => {
		render(MetadataManagerIsland);

		const manager = screen.getByTestId('metadata-manager');
		const artifacts = screen.getByTestId('metadata-artifacts');

		expect(manager.contains(artifacts)).toBe(true);
		// The artifacts drawer lives in the fields cell, below the form, and is
		// collapsed by default so primary fields stay the visual focus.
		expect(artifacts.closest('.metadata-fields-cell')).not.toBeNull();
		expect(screen.getByTestId('metadata-artifacts-toggle')).toHaveAttribute(
			'aria-expanded',
			'false',
		);
	});
});
