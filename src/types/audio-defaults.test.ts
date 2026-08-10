import { describe, expect, it } from 'vitest';
import { defaultEncoderSettings } from './audio';

describe('defaultEncoderSettings', () => {
	it('returns the complete runtime-auto encoder request defaults', () => {
		expect(defaultEncoderSettings()).toEqual({
			encoderType: 'auto',
			bitrateKbps: 64,
			bitrateMode: { mode: 'vbr', value: 3 },
			channels: 'auto',
			afterburner: true,
		});
	});
});
