import type { EncoderSettings } from '../../types/audio';

/** Total target kbps, or null when the encoder's quality setting owns bitrate. */
export function estimateKbpsFromSettings(settings: EncoderSettings): number | null {
	return settings.bitrateMode.mode === 'vbr' ? null : settings.bitrateKbps;
}
