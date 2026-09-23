import type { TitleAudioRequest } from '../../types/audio';

export function titleAudioRequest(overrides: Partial<TitleAudioRequest> = {}): TitleAudioRequest {
	return {
		format: 'm4b',
		intent: 'auto',
		sampleRate: 'auto',
		settings: {
			encoderType: 'native_aac',
			bitrateKbps: 64,
			bitrateMode: { mode: 'cbr' },
			channels: 'auto',
			afterburner: false,
			nativeAacSpeed: 0,
			faacProfile: 'auto',
		},
		...overrides,
	};
}
