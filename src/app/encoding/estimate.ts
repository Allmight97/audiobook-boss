import type { EncoderSettings } from '../../types/audio';

export function estimateKbpsFromSettings(settings: EncoderSettings): number | null {
	const { bitrateMode, bitrateKbps } = settings;
	// Quality-based VBR does not promise a bitrate; target-based modes do.
	return bitrateMode.mode === 'vbr' ? null : bitrateKbps;
}
