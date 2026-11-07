import type { TitleAudioRequest } from '../../lib/generated/tauri';

export function titleAudioRequest(): TitleAudioRequest {
	return {
		format: 'm4b',
		intent: 'auto',
		sampleRate: 'auto',
		settings: {
			encoderType: 'native_aac',
			bitrateKbps: 64,
			bitrateMode: { mode: 'cbr' },
			channels: 'auto',
			nativeAacSpeed: 0,
			faacProfile: 'auto',
		},
	};
}
