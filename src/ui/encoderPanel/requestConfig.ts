import type { EncodingRequestConfig } from '../../types/audio';
import { Atom } from '../../app/runtime/reactivity';
import { readEncodingRequestConfig } from './state';

const VBR_BITRATE_ESTIMATES: Record<number, number> = {
	1: 32,
	2: 48,
	3: 60,
	4: 72,
	5: 96,
};

export const encodingRequestConfigAtom = Atom.make<EncodingRequestConfig>(
	readEncodingRequestConfig(),
).pipe(Atom.keepAlive);

export function estimateKbpsFromRequest(config: EncodingRequestConfig): number {
	const { bitrateMode, bitrateKbps } = config.encoderSettings;
	if (bitrateMode.mode !== 'vbr') {
		return bitrateKbps;
	}
	return VBR_BITRATE_ESTIMATES[bitrateMode.value] ?? VBR_BITRATE_ESTIMATES[3] ?? bitrateKbps;
}

export const encodingEstimateBitrateKbpsAtom = Atom.make((get): number =>
	estimateKbpsFromRequest(get(encodingRequestConfigAtom)),
).pipe(Atom.keepAlive);

export function publishEncodingRequestConfig(set: (config: EncodingRequestConfig) => void): void {
	set(readEncodingRequestConfig());
}
