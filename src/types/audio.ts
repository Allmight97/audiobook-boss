// TypeScript interfaces for audio processing
import type {
	AudioFile as GeneratedAudioFile,
	FileListInfo as GeneratedFileListInfo,
	JobType as GeneratedJobType,
	OutputNamingConfig as GeneratedOutputNamingConfig,
	ProcessCommandResult as GeneratedProcessCommandResult,
	ProcessV2Payload as GeneratedProcessV2Payload,
	SampleRateConfig as GeneratedSampleRateConfig,
} from '../lib/generated/tauri';
import type { NullToOptionalDeep } from './ipc';

export type AudioFile = NullToOptionalDeep<GeneratedAudioFile>;

export type FileListInfo = NullToOptionalDeep<GeneratedFileListInfo>;

export type SampleRateConfig = GeneratedSampleRateConfig;

// Output naming options for folder/filename generation
export type OutputNamingConfig = GeneratedOutputNamingConfig;

// Combined UI output configuration used by the UI boundary
export interface OutputConfig {
	encoderSettings: EncoderSettings;
	sampleRate: SampleRateConfig;
	outputPath: string; // legacy field name; now represents the selected output folder
	outputNaming: OutputNamingConfig;
}

export interface ProcessingProgress {
	stage: ProcessingStage;
	progress: number;
	currentFile?: string;
	filesCompleted: number;
	totalFiles: number;
	etaSeconds?: number;
}

export type ProcessingStage =
	| 'Analyzing'
	| 'Converting'
	| 'Merging'
	| 'WritingMetadata'
	| 'Completed'
	| { Failed: string };

// Preview command typing helpers (Tauri boundary)
export interface PreviewRequest {
	previewSeconds?: number;
}

export interface ProcessCommandResult {
	message: NullToOptionalDeep<GeneratedProcessCommandResult>['message'];
	previewFilePath?: NullToOptionalDeep<GeneratedProcessCommandResult>['previewFilePath'];
	previewActualSeconds?: NullToOptionalDeep<GeneratedProcessCommandResult>['previewActualSeconds'];
	jobId?: NullToOptionalDeep<GeneratedProcessCommandResult>['jobId'];
}

// Encoder v2 types (Enhanced engine)
export type EncoderType = 'auto' | 'fdk_he_aac' | 'aac_at' | 'native_aac';
export type BitrateMode =
	| { mode: 'cbr' }
	| { mode: 'cvbr' }
	| { mode: 'vbr'; value: 1 | 2 | 3 | 4 | 5 };
export type ThreadSetting = { mode: 'auto' } | { mode: 'off' } | { mode: 'fixed'; value: number };
export type EncoderChannelConfig = 'auto' | 'mono' | 'stereo';

// Single source of truth for valid encoder bitrates (kbps)
// Matches Rust VALID_ENCODER_BITRATES in settings_encoder.rs
export const VALID_ENCODER_BITRATES = [48, 56, 64, 72, 80, 88, 96, 104, 112, 120, 128] as const;
export type BitrateKbps = (typeof VALID_ENCODER_BITRATES)[number];

export interface EncoderSettings {
	encoderType: EncoderType; // default: auto (resolver picks best available encoder)
	bitrateKbps: BitrateKbps;
	bitrateMode: BitrateMode;
	channels: EncoderChannelConfig;
	afterburner: boolean;
	threads: ThreadSetting;
	twoloop: boolean;
}

// Job Type for batch processing (Issue #81)
export type JobType = GeneratedJobType;

// Complete processing payload
export type ProcessV2Payload = Omit<NullToOptionalDeep<GeneratedProcessV2Payload>, 'settings'> & {
	settings: EncoderSettings;
};

// Default encoder settings with runtime auto resolution.
// Auto uses VBR by default to satisfy Rust boundary validation for `EncoderType::Auto`.
export const getDefaultEncoderSettingsForPlatform = (): EncoderSettings => {
	return {
		encoderType: 'auto',
		bitrateKbps: 64,
		bitrateMode: { mode: 'vbr', value: 3 },
		channels: 'auto',
		afterburner: false,
		threads: { mode: 'auto' },
		twoloop: true,
	};
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
