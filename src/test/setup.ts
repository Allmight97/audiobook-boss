/**
 * Vitest global setup file.
 *
 * This file runs before each test file and sets up:
 * - Tauri API mocks for testing outside the Tauri window
 * - DOM environment configuration
 * - Global test utilities
 */

import '@testing-library/jest-dom/vitest';
import { pathBasename } from '../lib/path/basename';
import { afterEach, beforeEach, vi } from 'vitest';
import { runtimeSettingsCapabilitiesFixture } from './fixtures/runtimeSettingsCapabilities';
import type {
	FileListInfo,
	MetadataIntentValidationResult,
	MetadataLookupResponse,
	MetadataSaveBatchResult,
	OperationListSnapshot,
	OperationSnapshot,
	ProcessCommandResult,
	RemoteIndexerConnection,
	SupportedAudioImportMetadata,
	WorkSubmissionAccepted,
} from '../lib/generated/tauri';

type TestEventHandler = (event: { event: string; id: number; payload: unknown }) => void;
type ProcessPayloadForTest = {
	inputFiles?: string[];
	jobType?: 'batch' | 'merge' | null;
};
const eventListeners = new Map<string, Set<TestEventHandler>>();
let mockJobCounter = 0;

function emitTestEvent(event: string, payload: unknown): void {
	const handlers = eventListeners.get(event);
	if (!handlers) return;
	for (const handler of handlers) {
		handler({ event, id: Date.now(), payload });
	}
}

function mockOperationSnapshot(
	operationId: string,
	kind: 'processingBatch' | 'processingMerge' | 'metadataSave',
	inputFiles: string[],
): OperationSnapshot {
	const isMetadataSave = kind === 'metadataSave';
	const title = isMetadataSave
		? `Metadata save (${inputFiles.length} files)`
		: kind === 'processingMerge'
			? `Merge encode (${inputFiles.length} files)`
			: `Batch encode (${inputFiles.length} files)`;
	const lanes = (
		isMetadataSave ? ['metadataWrite'] : ['analysis', 'encodeCpu', 'outputCommit']
	) as OperationSnapshot['lanes'];
	const childLane = (
		isMetadataSave ? 'metadataWrite' : 'encodeCpu'
	) as OperationSnapshot['lanes'][number];
	return {
		operationId,
		sequence: mockJobCounter,
		kind,
		status: 'accepted' as const,
		title,
		createdAtMs: Date.now(),
		startedAtMs: null,
		finishedAtMs: null,
		cancellable: true,
		cancelRequested: false,
		lanes,
		sourceInputIds: [],
		progress: {
			stage: 'pending' as const,
			percentage: 0,
			message: 'Accepted.',
			currentItemIndex: null,
			totalItems: inputFiles.length,
			bytesDownloaded: null,
			bytesTotal: null,
			etaSeconds: null,
		},
		children: inputFiles.map((path, index) => ({
			childJobId: isMetadataSave ? `metadata-${index}` : `input-${index}`,
			operationId,
			label: pathBasename(path, { fallback: 'path' }),
			status: 'queued' as const,
			lane: childLane,
			progress: {
				stage: 'pending' as const,
				percentage: 0,
				message: 'Queued.',
				currentItemIndex: null,
				totalItems: inputFiles.length,
				bytesDownloaded: null,
				bytesTotal: null,
				etaSeconds: null,
			},
			sourcePath: path,
			inputIndex: index,
			inputId: null,
			jobId: null,
			cancellable: false,
			cancelRequested: false,
			message: null,
		})),
		terminalSummary: null,
		warnings: [],
		errors: [],
		logTail: [],
	};
}

// Mock Tauri's invoke API
vi.mock('@tauri-apps/api/core', () => ({
	invoke: vi.fn().mockImplementation((cmd: string, _args?: unknown) => {
		switch (cmd) {
			case 'get_supported_audio_import_metadata':
				return Promise.resolve({
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
				} satisfies SupportedAudioImportMetadata);
			case 'discover_audio_import_paths': {
				const args = _args as { inputPaths?: string[] } | undefined;
				return Promise.resolve((args?.inputPaths ?? []) satisfies string[]);
			}
			case 'take_opened_audio_files':
				return Promise.resolve([] as string[]);
			case 'get_max_concurrent_jobs':
				return Promise.resolve(4);
			case 'get_runtime_settings_capabilities':
				return Promise.resolve(runtimeSettingsCapabilitiesFixture());
			case 'read_audio_cover_thumbnail':
				return Promise.resolve(null);
			case 'analyze_audio_files':
				return Promise.resolve({
					files: [
						{
							inputId: 'mock-input-1',
							path: '/mock/path/chapter1.mp3',
							size: 15 * 1024 * 1024,
							duration: 300,
							isValid: true,
							bitrate: 64,
							sampleRate: 44100,
							channels: 1,
							format: 'mp3',
							codecLabel: 'MP3',
							selectedDecoder: 'ffmpeg',
							tagTitle: null,
							tagArtist: null,
							error: null,
						},
						{
							inputId: 'mock-input-2',
							path: '/mock/path/chapter2.mp3',
							size: 20 * 1024 * 1024,
							duration: 400,
							isValid: true,
							bitrate: 64,
							sampleRate: 44100,
							channels: 1,
							format: 'mp3',
							codecLabel: 'MP3',
							selectedDecoder: 'ffmpeg',
							tagTitle: null,
							tagArtist: null,
							error: null,
						},
					],
					selectedDecoders: [
						{ decoderId: 'ffmpeg_mp3', decoderLabel: 'FFmpeg MP3' },
						{ decoderId: 'ffmpeg_mp3', decoderLabel: 'FFmpeg MP3' },
					],
					totalDuration: 700,
					totalSize: 35 * 1024 * 1024,
					validCount: 2,
					invalidCount: 0,
				} satisfies FileListInfo);
			case 'search_online_metadata':
				return Promise.resolve({
					results: [
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
							publishedDate: '2021',
							durationSeconds: 36000,
							coverUrl: 'https://covers.openlibrary.org/b/id/123456-L.jpg',
							audibleOnly: false,
						},
					],
					diagnostics: [],
				} satisfies MetadataLookupResponse);
			case 'process_audiobook_files': {
				mockJobCounter += 1;
				const jobId = `mock-job-${mockJobCounter}`;
				emitTestEvent('processing-queue', {
					operation_kind: 'processingBatch',
					items: [{ input_index: 0, file_path: '/mock/path/chapter1.mp3' }],
					max_concurrent: 2,
				});
				emitTestEvent('processing-progress', {
					operation_kind: 'processingBatch',
					stage: 'converting',
					percentage: 40,
					message: 'Converting audio',
					current_file: '/mock/path/chapter1.mp3',
					eta_seconds: 30,
					job_id: jobId,
					input_index: 0,
				});
				emitTestEvent('processing-progress', {
					operation_kind: 'processingBatch',
					stage: 'completed',
					percentage: 100,
					message: 'Processing completed successfully!',
					current_file: '/mock/path/chapter1.mp3',
					eta_seconds: 0,
					job_id: jobId,
					input_index: 0,
				});
				return Promise.resolve({
					jobType: 'batch',
					summary: {
						total: 1,
						succeeded: 1,
						skipped: 0,
						cancelled: 0,
						failed: 0,
					},
					terminalClass: 'success',
					results: [
						{
							inputIndex: 0,
							status: 'success' as const,
							message: 'Processing started (mock)',
							jobId,
							error: null,
							previewFilePath: null,
							previewActualSeconds: null,
						},
					],
				} satisfies ProcessCommandResult);
			}
			case 'submit_processing_operation': {
				const args = _args as
					| {
							request?: {
								payload?: ProcessPayloadForTest;
							};
					  }
					| undefined;
				const operationId = `mock-operation-${++mockJobCounter}`;
				const inputFiles = args?.request?.payload?.inputFiles ?? [];
				const kind =
					args?.request?.payload?.jobType === 'merge' ? 'processingMerge' : 'processingBatch';
				const snapshot = mockOperationSnapshot(operationId, kind, inputFiles);
				emitTestEvent('work-operation-snapshot', { snapshot });
				emitTestEvent('work-operation-list-snapshot', { operations: [snapshot] });
				return Promise.resolve({
					operationId,
					snapshot,
				} satisfies WorkSubmissionAccepted);
			}
			case 'list_work_operations':
				return Promise.resolve({
					operations: [],
				} satisfies OperationListSnapshot);
			case 'get_work_operation': {
				const args = _args as { operationId?: string } | undefined;
				const operationId = args?.operationId ?? 'mock-operation';
				return Promise.resolve(
					mockOperationSnapshot(operationId, 'processingBatch', []) satisfies OperationSnapshot,
				);
			}
			case 'cancel_work_operation': {
				const args = _args as { operationId?: string } | undefined;
				const operationId = args?.operationId ?? 'mock-operation';
				return Promise.resolve({
					...mockOperationSnapshot(operationId, 'processingBatch', []),
					status: 'cancelling' as const,
					cancelRequested: true,
					cancellable: false,
				} satisfies OperationSnapshot);
			}
			case 'save_metadata_batch': {
				mockJobCounter += 1;
				const operationId = `mock-metadata-operation-${mockJobCounter}`;
				const args = _args as
					| {
							items?: Array<{
								filePath: string;
								metadataPatch: Record<string, unknown>;
							}>;
					  }
					| undefined;
				const items = args?.items ?? [];
				const filePaths = items.map((item) => item.filePath);
				// Metadata save is a WorkRuntime MetadataSave operation: the Work
				// Center renders its snapshots while the command returns the
				// per-file result. Mirror that — emit running + terminal snapshots,
				// then resolve the synchronous result.
				const baseSnapshot = mockOperationSnapshot(operationId, 'metadataSave', filePaths);
				emitTestEvent('work-operation-snapshot', {
					snapshot: { ...baseSnapshot, status: 'running' as const, startedAtMs: Date.now() },
				});
				const terminalSnapshot: OperationSnapshot = {
					...baseSnapshot,
					status: 'completed' as const,
					startedAtMs: Date.now(),
					finishedAtMs: Date.now(),
					cancellable: false,
					progress: {
						...baseSnapshot.progress,
						stage: 'complete' as const,
						percentage: 100,
						message: `Completed ${items.length} item(s).`,
					},
					children: baseSnapshot.children.map((child) => ({
						...child,
						status: 'completed' as const,
						progress: { ...child.progress, stage: 'complete' as const, percentage: 100 },
						message: 'Metadata saved',
					})),
					terminalSummary: {
						total: items.length,
						succeeded: items.length,
						skipped: 0,
						cancelled: 0,
						failed: 0,
						message: `Completed ${items.length} item(s).`,
					},
				};
				emitTestEvent('work-operation-snapshot', { snapshot: terminalSnapshot });
				emitTestEvent('work-operation-list-snapshot', { operations: [terminalSnapshot] });
				return Promise.resolve({
					summary: {
						total: items.length,
						succeeded: items.length,
						skipped: 0,
						cancelled: 0,
						failed: 0,
					},
					results: items.map((item, index) => ({
						inputIndex: index,
						filePath: item.filePath,
						status: 'success' as const,
						message: 'Metadata saved',
						error: null,
					})),
				} satisfies MetadataSaveBatchResult);
			}
			case 'validate_metadata_intent_patch': {
				const args = _args as
					| {
							metadataPatch?: Record<string, unknown>;
					  }
					| undefined;
				return Promise.resolve({
					isValid: true,
					metadataPatch: args?.metadataPatch ?? {},
					fieldErrors: [],
				} satisfies MetadataIntentValidationResult);
			}
			case 'get_remote_source_indexer_connection':
				return Promise.resolve({
					baseUrl: null,
					categoryIds: [3030],
					apiKeyConfigured: false,
				} satisfies RemoteIndexerConnection);

			default:
				throw new Error(`[Test Mock] Unhandled Tauri invoke: ${cmd}`);
		}
	}),
	Channel: class MockChannel {
		// Minimal placeholder required by generated tauri-specta bindings import surface.
	},
}));

// Mock Tauri's event API
vi.mock('@tauri-apps/api/event', () => ({
	listen: vi.fn().mockImplementation((event: string, handler: TestEventHandler) => {
		if (!eventListeners.has(event)) {
			eventListeners.set(event, new Set());
		}
		eventListeners.get(event)?.add(handler);
		return Promise.resolve(() => {
			eventListeners.get(event)?.delete(handler);
		});
	}),
	emit: vi.fn().mockResolvedValue(undefined),
}));

// Mock Tauri's dialog plugin
vi.mock('@tauri-apps/plugin-dialog', () => ({
	open: vi.fn().mockResolvedValue(null),
	save: vi.fn().mockResolvedValue(null),
	message: vi.fn().mockResolvedValue(undefined),
	ask: vi.fn().mockResolvedValue(false),
	confirm: vi.fn().mockResolvedValue(false),
}));

// Mock Tauri's opener plugin
vi.mock('@tauri-apps/plugin-opener', () => ({
	openPath: vi.fn().mockResolvedValue(undefined),
	openUrl: vi.fn().mockResolvedValue(undefined),
}));

// Ensure browser-like runtime for tests (no embedded Tauri internals)
Object.defineProperty(window, '__TAURI_INTERNALS__', {
	value: undefined,
	writable: true,
});

// Keep DEV true in tests for consistent frontend test behavior
vi.stubEnv('DEV', true);

beforeEach(() => {
	eventListeners.clear();
	mockJobCounter = 0;
});

afterEach(() => {
	eventListeners.clear();
	mockJobCounter = 0;
});

const storage = new Map<string, string>();
const localStorageMock = {
	getItem: (key: string): string | null => (storage.has(key) ? storage.get(key)! : null),
	setItem: (key: string, value: string): void => {
		storage.set(key, value);
	},
	removeItem: (key: string): void => {
		storage.delete(key);
	},
	clear: (): void => {
		storage.clear();
	},
	key: (index: number): string | null => Array.from(storage.keys())[index] ?? null,
	get length(): number {
		return storage.size;
	},
};

Object.defineProperty(window, 'localStorage', {
	value: localStorageMock,
	configurable: true,
});
Object.defineProperty(globalThis, 'localStorage', {
	value: localStorageMock,
	configurable: true,
});
