import type {
	AudioIntent,
	AudiobookFormat,
	BitrateMode,
	CodecFamily,
	EncoderType,
	FormatOption,
} from '../../lib/generated/tauri';

const FORMATS: Record<AudiobookFormat, string> = {
	m4b: 'M4B',
	m4aOpus: 'M4A',
	mkaOpus: 'MKA',
};

const CODEC_FAMILIES: Record<CodecFamily, string> = {
	aac: 'AAC',
	opus: 'Opus',
};

const INTENTS: Record<AudioIntent, string> = {
	auto: 'Default',
	preserve: 'Keep original audio',
	encode: 'User Preference',
};

const ENCODERS: Record<EncoderType, string> = {
	auto: 'Auto',
	aac_at: 'Apple AAC',
	native_aac: 'Native AAC (NMR)',
	faac: 'FAAC',
	opus: 'Opus (libopus)',
};

export function formatOptionLabel(option: FormatOption): string {
	return `${FORMATS[option.format]} · ${CODEC_FAMILIES[option.codecFamily]}`;
}

export function intentLabel(intent: AudioIntent): string {
	return INTENTS[intent];
}

export function profileLabel(family: CodecFamily, encoder: EncoderType): string {
	if (family === 'opus') return CODEC_FAMILIES.opus;
	return encoder === 'faac' ? CODEC_FAMILIES.aac : 'AAC-LC';
}

export function encoderLabel(encoder: EncoderType): string {
	return ENCODERS[encoder];
}

export function bitrateModeLabel(mode: BitrateMode): string {
	switch (mode.mode) {
		case 'cbr':
			return 'CBR';
		case 'cvbr':
			return 'CVBR';
		case 'abr':
			return 'ABR';
		case 'vbr':
			return `VBR ${mode.value}`;
		case 'vbr_target':
			return 'VBR';
	}
}
