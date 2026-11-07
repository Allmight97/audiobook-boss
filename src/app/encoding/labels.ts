import type { BitrateMode, EncoderType } from '../../lib/generated/tauri';

const ENCODERS: Record<EncoderType, string> = {
	auto: 'Auto',
	aac_at: 'Apple AAC',
	native_aac: 'Native AAC (NMR)',
	faac: 'FAAC',
	opus: 'Opus (libopus)',
};

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
