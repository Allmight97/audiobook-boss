import { describe, expect, it } from 'vitest';
import { defaultEncoderSettings, type EncoderSettings } from './audio';
import { toBoundaryEncoderSettings } from './encoder';
import { runtimeSettingsCapabilitiesFixture } from '../test/fixtures/runtimeSettingsCapabilities';

const encoderCapabilities = () => runtimeSettingsCapabilitiesFixture().encoder;

describe('toBoundaryEncoderSettings', () => {
	it('passes valid generated FDK encoder settings through unchanged', () => {
		const defaults = {
			...defaultEncoderSettings(),
			encoderType: 'fdk_he_aac',
			afterburner: true,
		} satisfies EncoderSettings;

		expect(toBoundaryEncoderSettings(defaults, undefined, encoderCapabilities())).toEqual(defaults);
	});

	it('preserves afterburner on auto boundary payloads so auto-FDK keeps the user preference', () => {
		const boundary = {
			...defaultEncoderSettings(),
			encoderType: 'auto',
			afterburner: true,
		} satisfies EncoderSettings;

		expect(toBoundaryEncoderSettings(boundary, undefined, encoderCapabilities())).toEqual(boundary);
	});

	it('preserves a valid native boundary encoder payload unchanged', () => {
		const boundary = {
			encoderType: 'native_aac',
			bitrateKbps: 96,
			bitrateMode: { mode: 'cbr' },
			channels: 'stereo',
			afterburner: false,
		} satisfies EncoderSettings;

		const normalized = toBoundaryEncoderSettings(boundary, undefined, encoderCapabilities());

		expect(normalized).toEqual(boundary);
	});

	it('rejects malformed boundary encoder settings before they cross the Tauri boundary', () => {
		const defaults = defaultEncoderSettings();
		const malformed = {
			...defaults,
			encoderType: 'bogus',
			afterburner: undefined,
		} as unknown as EncoderSettings;

		const normalized = toBoundaryEncoderSettings(malformed, defaults, encoderCapabilities());

		expect(normalized.encoderType).toBe(defaults.encoderType);
		expect(normalized.afterburner).toBe(defaults.afterburner);
	});

	it('falls back to the default encoder when persisted UI flavor is invalid', () => {
		const defaults = defaultEncoderSettings();
		const normalized = toBoundaryEncoderSettings(
			{ flavor: 'bogus' as never },
			defaults,
			encoderCapabilities(),
		);

		expect(normalized.encoderType).toBe(defaults.encoderType);
	});

	it('preserves the FDK afterburner preference when UI flavor is auto', () => {
		const normalized = toBoundaryEncoderSettings(
			{
				flavor: 'auto',
				fdkAfterburner: true,
			},
			undefined,
			encoderCapabilities(),
		);

		expect(normalized.encoderType).toBe('auto');
		expect(normalized.afterburner).toBe(true);
	});

	it('uses backend capabilities to choose the default mode for a UI encoder flavor', () => {
		const normalized = toBoundaryEncoderSettings(
			{
				flavor: 'native_aac',
				bitrateMode: { mode: 'vbr', value: 3 },
			},
			undefined,
			encoderCapabilities(),
		);

		expect(normalized.encoderType).toBe('native_aac');
		expect(normalized.bitrateMode).toEqual({ mode: 'cbr' });
	});
});
