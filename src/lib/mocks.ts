import type { FileListInfo } from '../types/audio';
import type { AudiobookMetadata, OnlineMetadataResult } from '../types/metadata';
import {
	type ProcessingProgressEvent,
	type ProcessingQueueEvent,
	EVENTS,
	STAGES,
} from '../types/events';
import type { OpenDialogOptions } from '@tauri-apps/plugin-dialog';

// Mock Data
const MOCK_FILE_LIST: FileListInfo = {
	files: [
		{
			path: '/mock/path/chapter1.mp3',
			size: 15 * 1024 * 1024, // 15MB
			duration: 300, // 5 minutes
			isValid: true,
			bitrate: 64,
			sampleRate: 44100,
			channels: 1,
		},
		{
			path: '/mock/path/chapter2.mp3',
			size: 20 * 1024 * 1024, // 20MB
			duration: 400, // 6:40 minutes
			isValid: true,
			bitrate: 64,
			sampleRate: 44100,
			channels: 1,
		},
	],
	totalDuration: 700,
	totalSize: 35 * 1024 * 1024,
	validCount: 2,
	invalidCount: 0,
};

const MOCK_METADATA: AudiobookMetadata = {
	title: 'Mock Audiobook Title',
	artist: 'Mock Author',
	album: 'Mock Album',
	composer: 'Mock Narrator',
	date: 2023,
	genre: 'Audiobook',
	description: 'This is a mock description for testing purposes.',
	series: 'Mock Series',
	series_part: '1',
	subseries: 'Mock Sub-series',
	subseries_part: '1',
	cover_art: undefined,
};

const MOCK_LOOKUP_RESULTS: OnlineMetadataResult[] = [
	{
		source: 'audnexus',
		sourceId: 'OL12345W',
		title: 'Mock Lookup Title',
		authors: ['Mock Author'],
		narrators: ['Mock Narrator'],
		series: 'Mock Series',
		seriesPart: '1',
		subseries: 'Mock Sub-series',
		subseriesPart: '1',
		description: 'Mock description from lookup source.',
		publishedYear: 2021,
		durationSeconds: 36000,
		coverUrl: 'https://covers.openlibrary.org/b/id/123456-L.jpg',
		audibleOnly: false,
	},
	{
		source: 'audnexus',
		sourceId: '987654321',
		title: 'Mock Audnexus Result',
		authors: ['Mock Author'],
		narrators: [],
		series: undefined,
		seriesPart: undefined,
		subseries: undefined,
		subseriesPart: undefined,
		description: 'Mock Audnexus description.',
		publishedYear: 2020,
		durationSeconds: 32400,
		coverUrl: 'https://is1-ssl.mzstatic.com/image/thumb/Audio123/v4/cover.jpg',
		audibleOnly: true,
	},
];

const MOCK_COVER_ART_BYTES = [
	0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
	0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
	0x89, 0x00, 0x00, 0x00, 0x0a, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x00, 0x01, 0x00, 0x00,
	0x05, 0x00, 0x01, 0x0d, 0x0a, 0x2d, 0xb4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae,
	0x42, 0x60, 0x82,
];

// Event Simulation Helpers
type MockEventHandler = (event: { payload: unknown }) => void;
const listeners: Map<string, Set<MockEventHandler>> = new Map();
let mockJobCounter = 0;
let mockMaxConcurrent = 2;

export function mockListen(event: string, handler: MockEventHandler): Promise<() => void> {
	console.log(`[Bridge Mock] Listening for event: ${event}`);
	if (!listeners.has(event)) {
		listeners.set(event, new Set());
	}
	listeners.get(event)?.add(handler);

	return Promise.resolve(() => {
		console.log(`[Bridge Mock] Unlistening event: ${event}`);
		listeners.get(event)?.delete(handler);
	});
}

function emitEvent(event: string, payload: unknown) {
	const handlers = listeners.get(event);
	if (handlers) {
		for (const h of handlers) {
			h({ payload });
		}
	}
}

// Mock Command Implementations
export async function mockInvoke(cmd: string, args?: Record<string, unknown>): Promise<unknown> {
	console.log(`[Bridge Mock] Invoke: ${cmd}`, args);

	// Simulate network delay
	await new Promise((resolve) => setTimeout(resolve, 500));

	switch (cmd) {
		case 'analyze_audio_files':
			return MOCK_FILE_LIST as unknown;

		case 'read_audio_metadata':
			return MOCK_METADATA as unknown;

		case 'list_available_encoders':
			return {
				fdkAvailable: true,
				aacAtAvailable: true,
				nativeAacAvailable: true,
			} as unknown;

		case 'get_max_concurrent_jobs':
			return mockMaxConcurrent as unknown;

		case 'set_max_concurrent_jobs':
			mockMaxConcurrent = (args?.max_concurrent as number) ?? mockMaxConcurrent;
			return mockMaxConcurrent;

		case 'process_audiobook_files_v2':
			{
				const argsWithPayload = args?.payload as { inputFiles?: string[] } | undefined;
				const inputFiles: string[] = argsWithPayload?.inputFiles ?? [];
				emitEvent(EVENTS.QUEUE, {
					items: inputFiles.map((filePath, index) => ({
						input_index: index,
						file_path: filePath,
					})),
					max_concurrent: mockMaxConcurrent,
				} as ProcessingQueueEvent);

				if (inputFiles.length === 0) {
					mockJobCounter += 1;
					simulateProcessing(`mock-job-${mockJobCounter}`);
				} else {
					inputFiles.forEach((_filePath, index) => {
						mockJobCounter += 1;
						simulateProcessing(`mock-job-${mockJobCounter}`, index);
					});
				}
			}
			return {
				message: 'Processing started (mock)',
				jobId: `mock-job-${mockJobCounter}`,
			} as unknown;

		case 'cancel_processing':
			emitEvent(EVENTS.PROGRESS, {
				stage: STAGES.cancelled,
				percentage: 0,
				message: 'Cancelled by user',
				current_file: '',
				eta_seconds: 0,
                                job_id: args?.job_id as string | undefined,
			} as ProcessingProgressEvent);
			return undefined as unknown;

		case 'load_cover_art_file':
			// Return a small 1x1 transparent pixel as mock cover art
			return MOCK_COVER_ART_BYTES as unknown;

		case 'load_cover_art_from_url':
			return MOCK_COVER_ART_BYTES as unknown;

		case 'search_online_metadata':
			return MOCK_LOOKUP_RESULTS as unknown;

		default:
			console.warn(`[Bridge Mock] Unhandled command: ${cmd}`);
			return undefined as unknown;
	}
}

export async function mockOpen(options?: OpenDialogOptions): Promise<string | string[] | null> {
	console.log(`[Bridge Mock] Open Dialog`, options);
	if (options?.multiple) {
		return ['/mock/path/file1.mp3', '/mock/path/file2.mp3'];
	}
	return '/mock/path/selected_file.mp3';
}

export async function mockOpenExternal(path: string): Promise<void> {
	console.log(`[Bridge Mock] Open External: ${path}`);
	alert(`[Mock] Opening external path: ${path}`);
}

// Simulation Logic
function simulateProcessing(jobId: string, inputIndex?: number) {
	let progress = 0;
	const interval = setInterval(() => {
		progress += 10;

		if (progress > 100) {
			clearInterval(interval);
			emitEvent(EVENTS.PROGRESS, {
				stage: STAGES.completed,
				percentage: 100,
				message: 'Processing Complete',
				current_file: '',
				eta_seconds: 0,
				job_id: jobId,
				input_index: inputIndex,
			} as ProcessingProgressEvent);
		} else {
			emitEvent(EVENTS.PROGRESS, {
				stage: STAGES.converting,
				percentage: progress,
				message: `Processing... ${progress}%`,
				current_file: 'mock_file.mp3',
				eta_seconds: (100 - progress) / 10,
				job_id: jobId,
				input_index: inputIndex,
			} as ProcessingProgressEvent);
		}
	}, 500);
}
