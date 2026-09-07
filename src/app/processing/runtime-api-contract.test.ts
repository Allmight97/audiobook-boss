import { describe, expect, it } from 'vitest';

import * as processing from '.';

const EXPECTED_APP_PROCESSING_EXPORTS = ['createProcessingOwner'] as const;

describe('app Processing Public API Strip', () => {
	it('pins the app processing public export strip', () => {
		expect(Object.keys(processing).sort()).toEqual([...EXPECTED_APP_PROCESSING_EXPORTS].sort());
	});
});
