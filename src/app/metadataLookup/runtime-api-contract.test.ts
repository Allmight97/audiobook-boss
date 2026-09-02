import { describe, expect, it } from 'vitest';

import * as metadataLookup from '.';

const EXPECTED_APP_METADATA_LOOKUP_EXPORTS = ['createMetadataLookupOwner'] as const;

describe('app Metadata Lookup Public API Strip', () => {
	it('pins the app metadataLookup public export strip', () => {
		expect(Object.keys(metadataLookup).sort()).toEqual(
			[...EXPECTED_APP_METADATA_LOOKUP_EXPORTS].sort(),
		);
	});

	it('does not export cover-preview globals, bumpPreview, or workflow symbols', () => {
		expect(metadataLookup).not.toHaveProperty('bumpPreview');
		expect(metadataLookup).not.toHaveProperty('subscribeMetadataLookupCoverPreviews');
		expect(metadataLookup).not.toHaveProperty('scheduleMetadataLookupCoverPreviews');
		expect(metadataLookup).not.toHaveProperty('clearMetadataLookupCoverPreviewCache');
		expect(metadataLookup).not.toHaveProperty('cancelMetadataLookupCoverPreviewSchedule');
		expect(metadataLookup).not.toHaveProperty('getMetadataLookupCoverPreviewState');
		expect(metadataLookup).not.toHaveProperty('loadMetadataLookupCoverBytes');
		expect(metadataLookup).not.toHaveProperty('fetchMetadataLookupCoverPreview');
		expect(metadataLookup).not.toHaveProperty('makeMetadataLookupWorkflowServicesLayer');
		expect(metadataLookup).not.toHaveProperty('runMetadataLookupWorkflow');
	});
});
