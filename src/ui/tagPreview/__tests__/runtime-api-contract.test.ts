import { describe, expect, it } from 'vitest';

import * as tagPreview from '..';

const EXPECTED_TAG_PREVIEW_EXPORTS = ['TagPreviewView'] as const;

describe('Tag Preview public API contract', () => {
	it('pins the tag preview public export strip', () => {
		expect(Object.keys(tagPreview).sort()).toEqual([...EXPECTED_TAG_PREVIEW_EXPORTS].sort());
	});
});
