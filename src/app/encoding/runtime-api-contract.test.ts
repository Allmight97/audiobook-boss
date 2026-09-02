import { describe, expect, it } from 'vitest';

import * as encoding from '.';

const EXPECTED_APP_ENCODING_EXPORTS = ['createEncodingOwner'] as const;

describe('app Encoding Public API Strip', () => {
	it('pins the app encoding public export strip', () => {
		expect(Object.keys(encoding).sort()).toEqual([...EXPECTED_APP_ENCODING_EXPORTS].sort());
	});

	it('does not export bag, scheduler, or UI-global compatibility symbols', () => {
		expect(encoding).not.toHaveProperty('subscribeEncoderPanel');
		expect(encoding).not.toHaveProperty('readEncodingRequestConfig');
		expect(encoding).not.toHaveProperty('applyEncodingDefaults');
		expect(encoding).not.toHaveProperty('setFdkAfterburner');
		expect(encoding).not.toHaveProperty('encoderPanelState');
		expect(encoding).not.toHaveProperty('estimateKbpsFromRequest');
	});
});
