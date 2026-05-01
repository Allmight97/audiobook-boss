import { describe, expect, it } from 'vitest';
import { defaultEncoderSettings, VALID_ENCODER_BITRATES } from './audio';
import rustSettingsEncoderSource from '../../src-tauri/src/audio/settings_encoder.rs?raw';

describe('defaultEncoderSettings', () => {
	it('defaults to auto encoder with VBR mode', () => {
		const defaults = defaultEncoderSettings();
		expect(defaults.encoderType).toBe('auto');
		expect(defaults.bitrateMode).toEqual({ mode: 'vbr', value: 3 });
	});

	it('keeps frontend bitrate options aligned with the Rust boundary whitelist', () => {
		const match = rustSettingsEncoderSource.match(
			/VALID_ENCODER_BITRATES:\s*&\[u16\]\s*=\s*&\[(?<values>[^\]]+)\]/,
		);
		expect(match?.groups?.values).toBeDefined();

		const rustBitrates = match!
			.groups!.values.split(',')
			.map((value: string) => Number(value.trim()))
			.filter((value: number) => Number.isFinite(value));

		expect(VALID_ENCODER_BITRATES).toEqual(rustBitrates);
	});
});
