import type {
	AppSettings,
	AudioFile,
	FileListInfo,
	RemoteSourceAccountState,
	RemoteSourceProviderCapabilities,
	SupportedAudioImportMetadata,
} from '../lib/generated/tauri';
import { runtimeSettingsCapabilitiesFixture } from '../test/fixtures/runtimeSettingsCapabilities';

export const FIXTURE_LIBRARY_DIR = '/mock/library/Dune';
export const FIXTURE_OUTPUT_DIR = '/mock/exports/audiobooks';
export const FIXTURE_COVER_PATH = '/mock/library/Dune/cover.jpg';
export const FIXTURE_FFMPEG_PATH = '/mock/bin/ffmpeg';
export const FIXTURE_CHAPTER_1 = '/mock/library/Dune/01-Arrival.mp3';
export const FIXTURE_CHAPTER_2 = '/mock/library/Dune/02-Desert.mp3';
export const FIXTURE_INVALID = '/mock/library/Dune/broken.wav';

export const FIXTURE_AUDIO_PATHS = [FIXTURE_CHAPTER_1, FIXTURE_CHAPTER_2] as const;

export const MOCK_ENCODE_ERROR = {
	code: 'ffmpeg_error',
	category: 'processing',
	message: 'Mock encode failed: fixture encoder error. No files were written.',
	detail: 'ui:mock does not invoke Rust or write audio.',
} as const;

export const MOCK_AUTH_ERROR = {
	code: 'invalid_input',
	category: 'validation',
	message: 'Mock Audible account is logged out. No provider was contacted.',
	detail: null,
} as const;

export function supportedImportMetadata(): SupportedAudioImportMetadata {
	return {
		formats: [
			{ extension: 'mp3', label: 'MP3' },
			{ extension: 'm4a', label: 'M4A/M4B' },
			{ extension: 'm4b', label: 'M4A/M4B' },
			{ extension: 'aac', label: 'AAC' },
			{ extension: 'wav', label: 'WAV' },
			{ extension: 'flac', label: 'FLAC' },
		],
		extensions: ['mp3', 'm4a', 'm4b', 'aac', 'wav', 'flac'],
		formatsText: 'MP3, M4A/M4B, AAC, WAV, and FLAC',
		supportText: 'Supports MP3, M4A/M4B, AAC, WAV, and FLAC audio files',
	};
}

export function defaultMockSettings(): AppSettings {
	return {
		maxConcurrentJobs: { mode: 'auto' },
		encoderDefaults: {
			settings: {
				encoderType: 'auto',
				bitrateKbps: 64,
				bitrateMode: { mode: 'vbr', value: 3 },
				channels: 'auto',
				afterburner: true,
			},
			sampleRate: 'auto',
		},
		outputDefaults: {
			outputDirectory: FIXTURE_OUTPUT_DIR,
			outputNaming: { preset: 'absDefault', includeYear: false, customTemplate: null },
		},
		toolchain: { externalFfmpegPath: null },
		startupBehavior: 'rememberLastState',
		pinnedDefaults: null,
	};
}

export function runtimeCapabilities() {
	return runtimeSettingsCapabilitiesFixture();
}

export function audibleProvider(): RemoteSourceProviderCapabilities {
	return {
		providerId: 'audible',
		label: 'Audible',
		authFlow: 'externalBrowserHandoff',
		supportsLibraryScan: true,
		supportsPagedScan: false,
		supportsTypeaheadFilter: true,
		supportsSupplementalPdf: true,
		supportsMaterializedAudio: true,
		supportsRefresh: true,
		requiresLiveSession: true,
		knownUnsupportedReasons: [],
	};
}

export function loggedOutAudibleState(): RemoteSourceAccountState {
	return {
		providerId: 'audible',
		status: 'needsAuth',
		account: null,
		message: 'Mock runtime: Audible is logged out. Sign-in does not contact Audible.',
	};
}

export function fixtureAudioFile(path: string, overrides: Partial<AudioFile> = {}): AudioFile {
	const catalog: Record<string, AudioFile> = {
		[FIXTURE_CHAPTER_1]: {
			inputId: 'mock-input-1',
			path: FIXTURE_CHAPTER_1,
			size: 15 * 1024 * 1024,
			duration: 300,
			format: 'mp3',
			bitrate: 64,
			sampleRate: 44100,
			channels: 1,
			codecLabel: 'MP3',
			selectedDecoder: 'ffmpeg',
			tagTitle: 'Dune',
			tagArtist: 'Frank Herbert',
			isValid: true,
			error: null,
		},
		[FIXTURE_CHAPTER_2]: {
			inputId: 'mock-input-2',
			path: FIXTURE_CHAPTER_2,
			size: 20 * 1024 * 1024,
			duration: 400,
			format: 'mp3',
			bitrate: 64,
			sampleRate: 44100,
			channels: 1,
			codecLabel: 'MP3',
			selectedDecoder: 'ffmpeg',
			tagTitle: 'Dune',
			tagArtist: 'Frank Herbert',
			isValid: true,
			error: null,
		},
		[FIXTURE_INVALID]: {
			inputId: 'mock-input-invalid',
			path: FIXTURE_INVALID,
			size: 128,
			duration: null,
			format: 'wav',
			bitrate: null,
			sampleRate: null,
			channels: null,
			codecLabel: null,
			selectedDecoder: null,
			tagTitle: null,
			tagArtist: null,
			isValid: false,
			error: 'Mock fixture: this path is not a real audio file.',
		},
	};

	const base = catalog[path] ?? {
		inputId: `mock-input-${pathBasename(path)}`,
		path,
		size: 8 * 1024 * 1024,
		duration: 180,
		format: extensionOf(path),
		bitrate: 64,
		sampleRate: 44100,
		channels: 1,
		codecLabel: extensionOf(path).toUpperCase(),
		selectedDecoder: 'ffmpeg',
		tagTitle: pathBasename(path),
		tagArtist: 'Mock Author',
		isValid: true,
		error: null,
	};

	return { ...base, ...overrides, path: overrides.path ?? path };
}

export function analyzeFixturePaths(filePaths: readonly string[]): FileListInfo {
	const files = filePaths.map((path) => fixtureAudioFile(path));
	return {
		files,
		selectedDecoders: files.map((file) =>
			file.isValid ? { decoderId: 'ffmpeg_mp3', decoderLabel: 'FFmpeg MP3' } : null,
		),
		totalDuration: files.reduce((sum, file) => sum + (file.duration ?? 0), 0),
		totalSize: files.reduce((sum, file) => sum + (file.size ?? 0), 0),
		validCount: files.filter((file) => file.isValid).length,
		invalidCount: files.filter((file) => !file.isValid).length,
	};
}

export function discoverFixturePaths(inputPaths: readonly string[]): string[] {
	const discovered = new Set<string>();
	for (const input of inputPaths) {
		if (input === FIXTURE_LIBRARY_DIR) {
			for (const path of FIXTURE_AUDIO_PATHS) {
				discovered.add(path);
			}
			continue;
		}
		if (looksLikeAudioPath(input)) {
			discovered.add(input);
		}
	}
	return [...discovered];
}

function looksLikeAudioPath(path: string): boolean {
	return /\.(mp3|m4a|m4b|aac|wav|flac)$/i.test(path);
}

function extensionOf(path: string): string {
	const match = /\.([a-z0-9]+)$/i.exec(path);
	return match?.[1]?.toLowerCase() ?? 'mp3';
}

function pathBasename(path: string): string {
	const segments = path.split(/[\\/]/).filter(Boolean);
	return segments[segments.length - 1] ?? path;
}
