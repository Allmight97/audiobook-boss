// TypeScript interfaces for audio processing
import type {
	BitrateMode as GeneratedBitrateMode,
	ChannelConfig as GeneratedChannelConfig,
	AudioFile as GeneratedAudioFile,
	CollisionPolicy as GeneratedCollisionPolicy,
	DecoderSelection as GeneratedDecoderSelection,
	EncoderAvailability as GeneratedEncoderAvailability,
	EncoderCapabilitySource as GeneratedEncoderCapabilitySource,
	EncoderSettings as GeneratedEncoderSettings,
	EncoderType as GeneratedEncoderType,
	ExternalToolchainPreference as GeneratedExternalToolchainPreference,
	FileListInfo as GeneratedFileListInfo,
	JobType as GeneratedJobType,
	OutputCollisionInfo as GeneratedOutputCollisionInfo,
	OutputCollisionKind as GeneratedOutputCollisionKind,
	OutputKind as GeneratedOutputKind,
	OutputNamingConfig as GeneratedOutputNamingConfig,
	PlannedOutput as GeneratedPlannedOutput,
	PlannedOutputAction as GeneratedPlannedOutputAction,
	ProcessCommandResult as GeneratedProcessCommandResult,
	ProcessResultEntry as GeneratedProcessResultEntry,
	ProcessResultStatus as GeneratedProcessResultStatus,
	ProcessResultSummary as GeneratedProcessResultSummary,
	ProcessPayload as GeneratedProcessPayload,
	ProcessingPreflightPlan as GeneratedProcessingPreflightPlan,
	SampleRateConfig as GeneratedSampleRateConfig,
	ThreadSetting as GeneratedThreadSetting,
} from '../lib/generated/tauri';
import type { AppErrorEnvelope } from '../lib/tauri/appError';
import type { NullToOptionalDeep } from './ipc';

export type AudioFile = NullToOptionalDeep<GeneratedAudioFile>;
export type DecoderSelection = NullToOptionalDeep<GeneratedDecoderSelection>;

export type FileListInfo = NullToOptionalDeep<GeneratedFileListInfo>;
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
export type EncoderCapabilitySource = GeneratedEncoderCapabilitySource;
export type BitrateMode = GeneratedBitrateMode;
export type EncoderChannelConfig = GeneratedChannelConfig;
export type EncoderType = GeneratedEncoderType;
export type ThreadSetting = GeneratedThreadSetting;
export type EncoderSettings = GeneratedEncoderSettings;
export type ExternalToolchainPreference = NullToOptionalDeep<GeneratedExternalToolchainPreference>;

// Output naming options for folder/filename generation
export type OutputNamingConfig = NullToOptionalDeep<GeneratedOutputNamingConfig>;

// Combined UI output configuration used by the UI boundary
export interface OutputConfig {
	encoderSettings: EncoderSettings;
	toolchainSettings: ExternalToolchainPreference;
	sampleRate: SampleRateConfig;
	outputPath: string; // backend contract name; stores the selected output directory
	outputNaming: OutputNamingConfig;
}

// Preview command typing helpers (Tauri boundary)
export interface PreviewRequest {
	previewSeconds?: number;
}

export type ProcessResultStatus = GeneratedProcessResultStatus;
export type ProcessResultSummary = NullToOptionalDeep<GeneratedProcessResultSummary>;
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

// Single source of truth for valid encoder bitrates (kbps)
// Matches Rust VALID_ENCODER_BITRATES in settings_encoder.rs
export const VALID_ENCODER_BITRATES = [48, 56, 64, 72, 80, 88, 96, 104, 112, 120, 128] as const;
export type BitrateKbps = (typeof VALID_ENCODER_BITRATES)[number];

// Job Type for batch processing (Issue #81)
export type JobType = GeneratedJobType;

// Complete processing payload
export type ProcessPayload = Omit<NullToOptionalDeep<GeneratedProcessPayload>, 'settings'> & {
	settings: EncoderSettings;
};

// Default encoder settings with runtime auto resolution.
// Auto uses VBR by default to satisfy Rust boundary validation for `EncoderType::Auto`.
export const getDefaultEncoderSettingsForPlatform = (): EncoderSettings => {
	const defaults = {
		encoderType: 'auto',
		bitrateKbps: 64,
		bitrateMode: { mode: 'vbr', value: 3 },
		channels: 'auto',
		afterburner: true,
		threads: { mode: 'auto' },
		twoloop: true,
	} satisfies EncoderSettings;
	return defaults;
};

// Default encoder settings (delegates to platform-aware helper)
export const defaultEncoderSettings = (): EncoderSettings => getDefaultEncoderSettingsForPlatform();

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
