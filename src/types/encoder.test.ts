import { describe, expect, it } from 'vitest';
import { defaultEncoderSettings, type EncoderSettings } from './audio';
import { toBoundaryEncoderSettings } from './encoder';

describe('toBoundaryEncoderSettings', () => {
	it('passes valid generated FDK encoder settings through unchanged', () => {
		const defaults = {
			...defaultEncoderSettings(),
			encoderType: 'fdk_he_aac',
			afterburner: true,
		} satisfies EncoderSettings;

		expect(toBoundaryEncoderSettings(defaults)).toEqual(defaults);
	});

	it('preserves a valid boundary encoder payload when twoloop is omitted', () => {
		const boundary = {
			encoderType: 'native_aac',
			bitrateKbps: 96,
			bitrateMode: { mode: 'cbr' },
			channels: 'stereo',
			afterburner: false,
			threads: { mode: 'auto' },
		} satisfies EncoderSettings;

		const normalized = toBoundaryEncoderSettings(boundary);

		expect(normalized).toEqual(boundary);
		expect('twoloop' in normalized).toBe(false);
	});

	it('rejects malformed boundary encoder settings before they cross the Tauri boundary', () => {
		const defaults = defaultEncoderSettings();
		const malformed = {
			...defaults,
			encoderType: 'bogus',
			afterburner: undefined,
			threads: undefined,
			twoloop: undefined,
		} as unknown as EncoderSettings;

		const normalized = toBoundaryEncoderSettings(malformed, defaults);

		expect(normalized.encoderType).toBe(defaults.encoderType);
		expect(normalized.afterburner).toBe(false);
		expect(normalized.threads).toEqual(defaults.threads);
		expect(normalized.twoloop).toBe(defaults.twoloop);
	});

	it('falls back to the default encoder when persisted UI flavor is invalid', () => {
		const defaults = defaultEncoderSettings();
		const normalized = toBoundaryEncoderSettings({ flavor: 'bogus' as never }, defaults);

		expect(normalized.encoderType).toBe(defaults.encoderType);
	});
});
