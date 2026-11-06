import type {
	ChapterSpec as GeneratedAudioChapter,
	AudioFile as GeneratedAudioFile,
	CollisionPolicy as GeneratedCollisionPolicy,
	EncoderSettingsCapabilities as GeneratedEncoderSettingsCapabilities,
	EncoderSettings as GeneratedEncoderSettings,
	EncoderType as GeneratedEncoderType,
	FaacProfile as GeneratedFaacProfile,
	OutputCollisionKind as GeneratedOutputCollisionKind,
	OutputNamingConfig as GeneratedOutputNamingConfig,
	PlannedOutput as GeneratedPlannedOutput,
	ProcessCommandResult as GeneratedProcessCommandResult,
	ProcessResultEntry as GeneratedProcessResultEntry,
} from '../lib/generated/tauri';
import type { AppErrorEnvelope } from '../lib/tauri/appError';
import type { NullToOptionalDeep } from './ipc';

type GeneratedAudioFileUi = NullToOptionalDeep<GeneratedAudioFile>;
type AudioChapter = NullToOptionalDeep<GeneratedAudioChapter>;
export type AudioFile = Omit<GeneratedAudioFileUi, 'inputId' | 'chapters'> & {
	inputId?: string;
	chapters?: AudioChapter[];
};

export type CollisionPolicy = GeneratedCollisionPolicy;
export type OutputCollisionKind = GeneratedOutputCollisionKind;
export type PlannedOutput = NullToOptionalDeep<GeneratedPlannedOutput>;

export type EncoderType = GeneratedEncoderType;
export type FaacProfile = GeneratedFaacProfile;
export type EncoderSettings = GeneratedEncoderSettings;
export type EncoderSettingsCapabilities = NullToOptionalDeep<GeneratedEncoderSettingsCapabilities>;

export type OutputNamingConfig = NullToOptionalDeep<GeneratedOutputNamingConfig>;

type ProcessResultError = AppErrorEnvelope;
type ProcessCommandJobResult = Omit<NullToOptionalDeep<GeneratedProcessResultEntry>, 'error'> & {
	error?: ProcessResultError | null;
};
export type ProcessCommandResult = Omit<
	NullToOptionalDeep<GeneratedProcessCommandResult>,
	'results'
> & {
	results: ProcessCommandJobResult[];
};

export const formatDuration = (seconds: number | undefined): string => {
	if (seconds == null || Number.isNaN(seconds)) {
		return '---';
	}

	const hours = Math.floor(seconds / 3600);
	const minutes = Math.floor((seconds % 3600) / 60);
	const secs = Math.floor(seconds % 60);

	if (hours > 0) {
		return `${hours}:${minutes.toString().padStart(2, '0')}:${secs.toString().padStart(2, '0')}`;
	}
	return `${minutes}:${secs.toString().padStart(2, '0')}`;
};

export const formatFileSize = (bytes: number | undefined): string => {
	if (bytes == null || Number.isNaN(bytes)) {
		return '---';
	}

	const units = ['B', 'KB', 'MB', 'GB'];
	let size = bytes;
	let unitIndex = 0;

	while (size >= 1024 && unitIndex < units.length - 1) {
		size /= 1024;
		unitIndex++;
	}

	return `${size.toFixed(1)} ${units[unitIndex]}`;
};

/** Imported audio bitrate is bits per second; encoder targets use kilobits per second. */
export const formatAudioBitrate = (bitsPerSecond: number | undefined): string =>
	bitsPerSecond ? `${Math.round(bitsPerSecond / 100) / 10} kbps` : 'N/A';

export type {
	AudiobookFormat,
	AudioIntent,
	TitleAudioRequest,
	TitleAudioPlan,
} from '../lib/generated/tauri';

export {
	SUPPORTED_AUDIO_IMPORT_EXTENSIONS,
	SUPPORTED_AUDIO_IMPORT_SUPPORT_TEXT,
} from '../lib/generated/tauri';
