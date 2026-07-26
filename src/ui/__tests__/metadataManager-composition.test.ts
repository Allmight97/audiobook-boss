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
	it('arranges cover art and metadata form zones without the removed artifacts drawer', () => {
		render(MetadataManagerIsland);

		const manager = screen.getByTestId('metadata-manager');
		expect(manager.querySelector('.metadata-cover-cell')).not.toBeNull();
		expect(manager.querySelector('.metadata-fields-cell')).not.toBeNull();
		// The embedded-artifacts drawer was removed pending re-ideation; its
		// backend clear path stays contractual in metadataSession.
		expect(screen.queryByTestId('metadata-artifacts')).toBeNull();
	});
});
