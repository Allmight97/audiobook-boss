import { describe, expect, it } from 'vitest';

import * as encoding from '.';

const EXPECTED_APP_ENCODING_EXPORTS = ['createEncodingOwner'] as const;

describe('app Encoding Public API Strip', () => {
	it('pins the app encoding public export strip', () => {
		expect(Object.keys(encoding).sort()).toEqual([...EXPECTED_APP_ENCODING_EXPORTS].sort());
	});
});
