import type { TitleAudioRequest } from '../../types/audio';

type RequestOverrides = Partial<Pick<TitleAudioRequest, 'intent' | 'sampleRate'>>;

export function titleAudioRequest(overrides: RequestOverrides = {}): TitleAudioRequest {
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
		...overrides,
	};
}

export function mp3AudioRequest(overrides: RequestOverrides = {}): TitleAudioRequest {
	return { format: 'mp3', intent: 'auto', sampleRate: 'auto', ...overrides };
}
