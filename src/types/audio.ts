// TypeScript interfaces for audio processing
import type {
	ChapterSpec as GeneratedAudioChapter,
	AudioHandling as GeneratedAudioHandling,
	BitrateMode as GeneratedBitrateMode,
	ChannelConfig as GeneratedChannelConfig,
	AudioFile as GeneratedAudioFile,
	CollisionPolicy as GeneratedCollisionPolicy,
	EncoderAvailability as GeneratedEncoderAvailability,
	EncoderConfigurationCapability as GeneratedEncoderConfigurationCapability,
	EncoderSettingsCapabilities as GeneratedEncoderSettingsCapabilities,
	EncoderSettings as GeneratedEncoderSettings,
	EncoderType as GeneratedEncoderType,
	FaacProfile as GeneratedFaacProfile,
	BitrateModeKind as GeneratedBitrateModeKind,
	MaxConcurrentJobsCapabilities as GeneratedMaxConcurrentJobsCapabilities,
	OutputCollisionInfo as GeneratedOutputCollisionInfo,
	OutputCollisionKind as GeneratedOutputCollisionKind,
	OutputKind as GeneratedOutputKind,
	OutputNamingConfig as GeneratedOutputNamingConfig,
	PlannedOutput as GeneratedPlannedOutput,
	PlannedOutputAction as GeneratedPlannedOutputAction,
	OperationResultSummary as GeneratedOperationResultSummary,
	ProcessCommandResult as GeneratedProcessCommandResult,
	ProcessResultEntry as GeneratedProcessResultEntry,
	ProcessResultStatus as GeneratedProcessResultStatus,
	SampleRateConfig as GeneratedSampleRateConfig,
	SupportedAudioImportMetadata as GeneratedSupportedAudioImportMetadata,
} from '../lib/generated/tauri';
import type { AppErrorEnvelope } from '../lib/tauri/appError';
import type { NullToOptionalDeep } from './ipc';

type GeneratedAudioFileUi = NullToOptionalDeep<GeneratedAudioFile>;
export type AudioHandling = GeneratedAudioHandling;
export type AudioChapter = NullToOptionalDeep<GeneratedAudioChapter>;
export type AudioFile = Omit<GeneratedAudioFileUi, 'inputId' | 'chapters'> & {
	inputId?: string;
	chapters?: AudioChapter[];
};

export type CollisionPolicy = GeneratedCollisionPolicy;
export type OutputKind = GeneratedOutputKind;
export type OutputCollisionKind = GeneratedOutputCollisionKind;
export type OutputCollisionInfo = NullToOptionalDeep<GeneratedOutputCollisionInfo>;
export type PlannedOutputAction = GeneratedPlannedOutputAction;
export type PlannedOutput = NullToOptionalDeep<GeneratedPlannedOutput>;

export type SampleRateConfig = GeneratedSampleRateConfig;
export type EncoderAvailability = NullToOptionalDeep<GeneratedEncoderAvailability>;
export type EncoderConfigurationCapability = GeneratedEncoderConfigurationCapability;
export type BitrateMode = GeneratedBitrateMode;
export type BitrateModeKind = GeneratedBitrateModeKind;
export type EncoderChannelConfig = GeneratedChannelConfig;
export type EncoderType = GeneratedEncoderType;
export type FaacProfile = GeneratedFaacProfile;
export type EncoderSettings = GeneratedEncoderSettings;
export type EncoderSettingsCapabilities = NullToOptionalDeep<GeneratedEncoderSettingsCapabilities>;
export type MaxConcurrentJobsCapabilities =
	NullToOptionalDeep<GeneratedMaxConcurrentJobsCapabilities>;
export type SupportedAudioImportMetadata = GeneratedSupportedAudioImportMetadata;

// Output naming options for folder/filename generation
export type OutputNamingConfig = NullToOptionalDeep<GeneratedOutputNamingConfig>;

export type ProcessResultStatus = GeneratedProcessResultStatus;
export type ProcessResultSummary = NullToOptionalDeep<GeneratedOperationResultSummary>;
export type ProcessResultError = AppErrorEnvelope;
export type ProcessCommandJobResult = Omit<
	NullToOptionalDeep<GeneratedProcessResultEntry>,
	'error'
> & {
	error?: ProcessResultError | null;
};
export type ProcessCommandResult = Omit<
	NullToOptionalDeep<GeneratedProcessCommandResult>,
	'results'
> & {
	results: ProcessCommandJobResult[];
};

export type BitrateKbps = EncoderSettings['bitrateKbps'];

// Utility functions
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
