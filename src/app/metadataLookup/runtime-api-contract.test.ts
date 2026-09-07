import { describe, expect, it } from 'vitest';

import * as metadataLookup from '.';

const EXPECTED_APP_METADATA_LOOKUP_EXPORTS = ['createMetadataLookupOwner'] as const;

describe('app Metadata Lookup Public API Strip', () => {
	it('pins the app metadataLookup public export strip', () => {
		expect(Object.keys(metadataLookup).sort()).toEqual(
			[...EXPECTED_APP_METADATA_LOOKUP_EXPORTS].sort(),
		);
	});
});
