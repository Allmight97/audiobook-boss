import { describe, expect, it } from 'vitest';
import * as encoderPanel from '..';

const EXPECTED_ENCODER_PANEL_EXPORTS = [
	'applyEncodingDefaults',
	'readEncoderDefaultsFromState',
	'readEncoderSummaryLabel',
	'readEncodingRequestConfig',
	'readFdkAfterburner',
	'setFdkAfterburner',
] as const;

describe('Encoder Panel Runtime public API contract', () => {
	it('pins the encoder panel public export strip', () => {
		expect(Object.keys(encoderPanel).sort()).toEqual([...EXPECTED_ENCODER_PANEL_EXPORTS].sort());
	});
});
