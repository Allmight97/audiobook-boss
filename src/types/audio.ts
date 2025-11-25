// TypeScript interfaces for audio processing

export interface AudioFile {
  path: string;
  size?: number;
  duration?: number;
  format?: string;
  bitrate?: number;
  sampleRate?: number;
  channels?: number;
  isValid: boolean;
  error?: string;
}

export interface FileListInfo {
  files: AudioFile[];
  totalDuration: number;
  totalSize: number;
  validCount: number;
  invalidCount: number;
}

export interface AudioSettings {
  bitrate: number;
  channels: ChannelConfig;
  sampleRate: SampleRateConfig;
  outputPath: string;
}

export type SampleRateConfig = 'auto' | { explicit: number };

export type ChannelConfig = 'Mono' | 'Stereo';

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
  message: string;
  previewFilePath?: string;
}

// Encoder v2 types (Phase 1)
export type EncoderType = 'aac_at' | 'he_aac_v1' | 'he_aac_v2';
export type AacCoder = 'twoloop' | 'fast';
export type ThreadSetting =
  | { mode: 'auto' }
  | { mode: 'off' }
  | { mode: 'fixed'; value: number };

// Single source of truth for valid encoder bitrates (kbps)
// Matches Rust VALID_ENCODER_BITRATES in settings_encoder.rs
export const VALID_ENCODER_BITRATES = [48, 56, 64, 72, 80, 88, 96, 112, 128] as const;
export type BitrateKbps = typeof VALID_ENCODER_BITRATES[number];

export interface EncoderSettings {
  encoderType: EncoderType;           // default: 'aac_at' on macOS
  bitrateKbps: BitrateKbps;
  channels: 1 | 2;                    // if he_aac_v2 → coerced to 2
  aacCoder?: AacCoder;                // ignored for aac_at
  afterburner?: boolean;              // ignored for aac_at
  threads: ThreadSetting;             // best-effort for aac_at
}

// Platform-aware default encoder settings
export const getDefaultEncoderSettingsForPlatform = (): EncoderSettings => {
  const isMac = typeof navigator !== 'undefined' && /Mac/i.test(navigator.userAgent);
  const encoderType: EncoderType = isMac ? 'aac_at' : 'he_aac_v1';
  return {
    encoderType,
    bitrateKbps: 64,
    channels: 1,
    aacCoder: 'twoloop',
    afterburner: false,
    threads: { mode: 'auto' }
  };
};

// Default encoder settings (delegates to platform-aware helper)
export const defaultEncoderSettings = (): EncoderSettings => getDefaultEncoderSettingsForPlatform();

// Audio settings presets
export const AudioPresets = {
  audiobook: (): AudioSettings => ({
    bitrate: 64,
    channels: 'Mono',
    sampleRate: { explicit: 22050 },
    outputPath: 'audiobook.m4b'
  }),
  
  highQuality: (): AudioSettings => ({
    bitrate: 128,
    channels: 'Stereo', 
    sampleRate: { explicit: 44100 },
    outputPath: 'audiobook_hq.m4b'
  }),
  
  lowBandwidth: (): AudioSettings => ({
    bitrate: 48,
    channels: 'Mono',
    sampleRate: { explicit: 22050 },
    outputPath: 'audiobook_low.m4b'
  })
};

// Utility functions
export const formatDuration = (seconds: number | undefined): string => {
  if (seconds == null || isNaN(seconds)) {
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
  if (bytes == null || isNaN(bytes)) {
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
