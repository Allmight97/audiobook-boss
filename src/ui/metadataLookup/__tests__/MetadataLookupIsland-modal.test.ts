import { fireEvent, render } from '@testing-library/svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const { closeMetadataLookupMock } = vi.hoisted(() => ({
	closeMetadataLookupMock: vi.fn(),
}));

vi.mock('../actions', () => ({
	applyMetadataLookupResult: vi.fn(),
	closeMetadataLookup: closeMetadataLookupMock,
	initMetadataLookup: vi.fn(),
	searchMetadataLookup: vi.fn(),
	skipMetadataLookupQueueItem: vi.fn(),
	useManualMetadataEntryFromLookup: vi.fn(),
}));

vi.mock('../metadataLookupCoverPreview.svelte', () => ({
	cancelMetadataLookupCoverPreviewSchedule: vi.fn(),
	getMetadataLookupCoverPreviewState: vi.fn(),
	scheduleMetadataLookupCoverPreviews: vi.fn(),
}));

vi.mock('../../../lib/tauri/client', () => ({
	tauriClient: { loadCoverArtFromUrl: vi.fn() },
}));

import MetadataLookupIsland from '../MetadataLookupIsland.svelte';
import { metadataLookupState } from '../state.svelte';

describe('MetadataLookupIsland modal wiring', () => {
	beforeEach(() => {
		closeMetadataLookupMock.mockClear();
		metadataLookupState.isOpen = true;
	});

	// This dialog's Close button has no in-flight guard today (it is never
	// disabled), so Escape and Close both route to the same unconditional
	// `closeMetadataLookup` callback.
	it('routes Escape through the same close callback the Close button uses', async () => {
		render(MetadataLookupIsland);

		await fireEvent.keyDown(document.getElementById('metadata-lookup-close') as Element, {
			key: 'Escape',
		});

		expect(closeMetadataLookupMock).toHaveBeenCalledTimes(1);
	});
});
