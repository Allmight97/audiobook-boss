// TypeScript interfaces for audio processing
import type {
	BitrateMode as GeneratedBitrateMode,
	ChannelConfig as GeneratedChannelConfig,
	AudioFile as GeneratedAudioFile,
	CollisionPolicy as GeneratedCollisionPolicy,
	DecoderSelection as GeneratedDecoderSelection,
	EncoderAvailability as GeneratedEncoderAvailability,
	EncoderBitrateModeCapability as GeneratedEncoderBitrateModeCapability,
	EncoderCapabilitySource as GeneratedEncoderCapabilitySource,
	EncoderSettingsCapabilities as GeneratedEncoderSettingsCapabilities,
	EncoderSettings as GeneratedEncoderSettings,
	EncoderType as GeneratedEncoderType,
	BitrateModeKind as GeneratedBitrateModeKind,
	FileListInfo as GeneratedFileListInfo,
	JobType as GeneratedJobType,
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
	ProcessPayload as GeneratedProcessPayload,
	ProcessingPreflightPlan as GeneratedProcessingPreflightPlan,
	RuntimeSettingsCapabilities as GeneratedRuntimeSettingsCapabilities,
	SampleRateConfig as GeneratedSampleRateConfig,
	SupplementalProcessingAsset as GeneratedSupplementalProcessingAsset,
	SupportedAudioImportMetadata as GeneratedSupportedAudioImportMetadata,
	ThreadSetting as GeneratedThreadSetting,
} from '../lib/generated/tauri';
import type { AppErrorEnvelope } from '../lib/tauri/appError';
import type { NullToOptionalDeep } from './ipc';

type GeneratedAudioFileUi = NullToOptionalDeep<GeneratedAudioFile>;
export type AudioFile = Omit<GeneratedAudioFileUi, 'inputId'> & {
	inputId?: string;
};
export type DecoderSelection = NullToOptionalDeep<GeneratedDecoderSelection>;

export type FileListInfo = Omit<NullToOptionalDeep<GeneratedFileListInfo>, 'files'> & {
	files: AudioFile[];
};
export type CollisionPolicy = GeneratedCollisionPolicy;
export type OutputKind = GeneratedOutputKind;
export type OutputCollisionKind = GeneratedOutputCollisionKind;
export type OutputCollisionInfo = NullToOptionalDeep<GeneratedOutputCollisionInfo>;
export type PlannedOutputAction = GeneratedPlannedOutputAction;
export type PlannedOutput = NullToOptionalDeep<GeneratedPlannedOutput>;
export type ProcessingPreflightPlan = Omit<
	NullToOptionalDeep<GeneratedProcessingPreflightPlan>,
	'outputs'
> & {
	outputs: PlannedOutput[];
};

export type SampleRateConfig = GeneratedSampleRateConfig;
export type EncoderAvailability = NullToOptionalDeep<GeneratedEncoderAvailability>;
export type EncoderBitrateModeCapability = GeneratedEncoderBitrateModeCapability;
export type EncoderCapabilitySource = GeneratedEncoderCapabilitySource;
export type BitrateMode = GeneratedBitrateMode;
export type BitrateModeKind = GeneratedBitrateModeKind;
export type EncoderChannelConfig = GeneratedChannelConfig;
export type EncoderType = GeneratedEncoderType;
export type ThreadSetting = GeneratedThreadSetting;
export type EncoderSettings = GeneratedEncoderSettings;
export type EncoderSettingsCapabilities = NullToOptionalDeep<GeneratedEncoderSettingsCapabilities>;
export type MaxConcurrentJobsCapabilities =
	NullToOptionalDeep<GeneratedMaxConcurrentJobsCapabilities>;
export type RuntimeSettingsCapabilities = NullToOptionalDeep<GeneratedRuntimeSettingsCapabilities>;
export type SupportedAudioImportMetadata = GeneratedSupportedAudioImportMetadata;

// Output naming options for folder/filename generation
export type OutputNamingConfig = NullToOptionalDeep<GeneratedOutputNamingConfig>;

export interface EncodingRequestConfig {
	encoderSettings: EncoderSettings;
	sampleRate: SampleRateConfig;
}

export interface OutputRequestConfig {
	outputDirectory: string;
	outputNaming: OutputNamingConfig;
}

// Combined UI process configuration composed at the processing workflow boundary.
export type ProcessingRequestConfig = EncodingRequestConfig & OutputRequestConfig;

// Preview command typing helpers (Tauri boundary)
export interface PreviewRequest {
	previewSeconds?: number;
}

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

// Job Type for batch processing (Issue #81)
export type JobType = GeneratedJobType;

// Complete processing payload
export type ProcessPayload = Omit<NullToOptionalDeep<GeneratedProcessPayload>, 'settings'> & {
	settings: EncoderSettings;
};
export type SupplementalProcessingAsset = NullToOptionalDeep<GeneratedSupplementalProcessingAsset>;

// Default encoder settings with runtime auto resolution.
// Auto uses VBR by default to satisfy Rust boundary validation for `EncoderType::Auto`.
export const defaultEncoderSettings = (): EncoderSettings => ({
	encoderType: 'auto',
	bitrateKbps: 64,
	bitrateMode: { mode: 'vbr', value: 3 },
	channels: 'auto',
	afterburner: true,
	threads: { mode: 'auto' },
	twoloop: true,
});

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
