import { encoderPanelState } from './state';

export function encoderLabel(value: string): string {
	switch (value) {
		case 'auto':
			return encoderPanelState.autoOptionLabel;
		case 'fdk_he_aac':
			return 'FDK AAC';
		case 'aac_at':
			return 'Apple AAC';
		case 'native_aac':
			return 'Native AAC (FFmpeg)';
		default:
			return value;
	}
}

export function bitrateModeLabel(value: string): string {
	return value.toUpperCase();
}

export function qualityLabel(value: number): string {
	if (value === encoderPanelState.capabilities?.vbrLevelMin) return `${value} (Smallest)`;
	if (value === encoderPanelState.capabilities?.vbrLevelDefault) return `${value} (Recommended)`;
	if (value === encoderPanelState.capabilities?.vbrLevelMax) return `${value} (Largest)`;
	return String(value);
}

export function sampleRateLabel(value: string): string {
	return value === 'auto' ? 'Auto' : `${value} Hz`;
}

export function channelLabel(value: string): string {
	switch (value) {
		case 'auto':
			return 'Auto';
		case 'mono':
			return 'Mono';
		case 'stereo':
			return 'Stereo';
		default:
			return value;
	}
}

export function sampleRateDetailText(): string {
	if (encoderPanelState.sampleRateSelection === 'auto') {
		return encoderPanelState.sampleRateAutoHint;
	}
	return `Using ${sampleRateLabel(encoderPanelState.sampleRateSelection)}.`;
}

export function channelsDetailText(): string {
	if (encoderPanelState.channelsSelection === 'auto') {
		return encoderPanelState.channelsAutoHint;
	}
	return `Using ${channelLabel(encoderPanelState.channelsSelection ?? 'auto')}.`;
}

export function encoderOptionDisabled(value: string): boolean {
	if (value === 'auto') return false;
	return Boolean(
		encoderPanelState.disabledEncoderOptions[
			value as keyof typeof encoderPanelState.disabledEncoderOptions
		],
	);
}
