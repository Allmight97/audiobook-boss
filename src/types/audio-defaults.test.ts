import { describe, expect, it } from 'vitest';
import { defaultEncoderSettings } from './audio';

describe('defaultEncoderSettings', () => {
	it('defaults to auto encoder with VBR mode', () => {
		const defaults = defaultEncoderSettings();
		expect(defaults.encoderType).toBe('auto');
		expect(defaults.bitrateMode).toEqual({ mode: 'vbr', value: 3 });
	});

	it('does not expose frontend-owned bitrate option mirrors', async () => {
		const audioTypes = await import('./audio');
		expect('VALID_ENCODER_BITRATES' in audioTypes).toBe(false);
	});
});
