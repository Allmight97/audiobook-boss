import type { AudioChoiceFacts, AudioEdit } from '../../lib/generated/tauri';

export type EncodingField = AudioEdit['field'];

function offered<T extends string>(list: readonly T[], value: string): T | null {
	return list.find((item) => item === value) ?? null;
}

function listedEdit(
	field: 'format' | 'intent' | 'encoder' | 'faacProfile' | 'rateControl' | 'channels',
	value: string,
	facts: AudioChoiceFacts,
): AudioEdit | null {
	switch (field) {
		case 'format': {
			const format = offered(
				facts.formatOptions.map((option) => option.format),
				value,
			);
			return format ? { field, value: format } : null;
		}
		case 'intent': {
			const intent = offered(facts.intentOptions, value);
			return intent ? { field, value: intent } : null;
		}
		case 'encoder': {
			const encoder = offered(
				facts.encoderOptions.map((option) => option.encoder),
				value,
			);
			return encoder ? { field, value: encoder } : null;
		}
		case 'faacProfile': {
			const profile = offered(facts.faacProfiles, value);
			return profile ? { field, value: profile } : null;
		}
		case 'rateControl': {
			const mode = offered(['abr', 'vbr'] as const, value);
			return mode && facts.allowedModes.includes(mode) ? { field, value: mode } : null;
		}
		case 'channels': {
			const channels = offered(facts.channelOptions, value);
			return channels ? { field, value: channels } : null;
		}
	}
}

function numberEdit(
	field: 'quality' | 'nativeSpeed' | 'bitrate' | 'sampleRate',
	value: string,
	facts: AudioChoiceFacts,
): AudioEdit | null {
	if (field === 'sampleRate' && value === 'auto') return { field, value };
	const number = Number(value);
	if (value.trim() === '' || !Number.isInteger(number) || number < 0) return null;
	switch (field) {
		case 'sampleRate':
			return { field, value: { explicit: number } };
		case 'nativeSpeed':
			return facts.nativeSpeedOptions.includes(number) ? { field, value: number } : null;
		case 'quality':
		case 'bitrate':
			return { field, value: number };
	}
}

export function editFor(
	field: EncodingField,
	value: string,
	facts: AudioChoiceFacts,
): AudioEdit | null {
	switch (field) {
		case 'quality':
		case 'nativeSpeed':
		case 'bitrate':
		case 'sampleRate':
			return numberEdit(field, value, facts);
		default:
			return listedEdit(field, value, facts);
	}
}
