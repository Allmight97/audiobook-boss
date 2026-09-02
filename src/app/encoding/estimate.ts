import type { EncodingRequestConfig } from '../../types/audio';

const VBR_BITRATE_ESTIMATES: Record<number, number> = {
	1: 32,
	2: 48,
	3: 60,
	4: 72,
	5: 96,
};

export function estimateKbpsFromRequest(config: EncodingRequestConfig): number {
	const { bitrateMode, bitrateKbps } = config.encoderSettings;
	if (bitrateMode.mode !== 'vbr') {
		return bitrateKbps;
	}
	return VBR_BITRATE_ESTIMATES[bitrateMode.value] ?? VBR_BITRATE_ESTIMATES[3] ?? bitrateKbps;
}
