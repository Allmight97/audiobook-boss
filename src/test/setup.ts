/**
 * Vitest global setup file.
 *
 * This file runs before each test file and sets up:
 * - Tauri API mocks for testing outside the Tauri window
 * - DOM environment configuration
 * - Global test utilities
 */

import '@testing-library/jest-dom/vitest';
import { afterEach, beforeEach, vi } from 'vitest';
import { runtimeSettingsCapabilitiesFixture } from './fixtures/runtimeSettingsCapabilities';

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
	kind: 'processingBatch' | 'processingMerge',
	inputFiles: string[],
) {
	return {
		operationId,
		sequence: mockJobCounter,
		kind,
		status: 'accepted',
		title:
			kind === 'processingMerge'
				? `Merge encode (${inputFiles.length} files)`
				: `Batch encode (${inputFiles.length} files)`,
		createdAtMs: Date.now(),
		startedAtMs: null,
		finishedAtMs: null,
		cancellable: true,
		cancelRequested: false,
		lanes: ['analysis', 'encodeCpu', 'outputCommit'],
		sourceInputIds: [],
		progress: {
			stage: 'pending',
			percentage: 0,
			message: 'Accepted.',
			currentItemIndex: null,
			totalItems: inputFiles.length,
			bytesDownloaded: null,
			bytesTotal: null,
			etaSeconds: null,
		},
		children: inputFiles.map((path, index) => ({
			childJobId: `input-${index}`,
			operationId,
			label: path.split(/[\\/]/).pop() || path,
			status: 'queued',
			lane: 'encodeCpu',
			progress: {
				stage: 'pending',
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
				});
			case 'discover_audio_import_paths': {
				const args = _args as { inputPaths?: string[] } | undefined;
				return Promise.resolve(args?.inputPaths ?? []);
			}
			case 'take_opened_audio_files':
				return Promise.resolve([]);
			case 'get_runtime_settings_capabilities':
				return Promise.resolve(runtimeSettingsCapabilitiesFixture());
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
						},
					],
					totalDuration: 700,
					totalSize: 35 * 1024 * 1024,
					validCount: 2,
					invalidCount: 0,
				});
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
				});
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
					results: [
						{
							inputIndex: 0,
							status: 'success',
							message: 'Processing started (mock)',
							jobId,
							error: null,
							previewFilePath: null,
							previewActualSeconds: null,
						},
					],
				});
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
				return Promise.resolve({ operationId, snapshot });
			}
			case 'list_work_operations':
				return Promise.resolve({ operations: [] });
			case 'get_work_operation': {
				const args = _args as { operationId?: string } | undefined;
				const operationId = args?.operationId ?? 'mock-operation';
				return Promise.resolve(mockOperationSnapshot(operationId, 'processingBatch', []));
			}
			case 'cancel_work_operation': {
				const args = _args as { operationId?: string } | undefined;
				const operationId = args?.operationId ?? 'mock-operation';
				return Promise.resolve({
					...mockOperationSnapshot(operationId, 'processingBatch', []),
					status: 'cancelling',
					cancelRequested: true,
					cancellable: false,
				});
			}
			case 'save_metadata_batch': {
				mockJobCounter += 1;
				const jobId = `mock-metadata-save-${mockJobCounter}`;
				const args = _args as
					| {
							items?: Array<{
								filePath: string;
								metadataPatch: Record<string, unknown>;
							}>;
					  }
					| undefined;
				const items = args?.items ?? [];
				emitTestEvent('processing-queue', {
					operation_kind: 'metadataSave',
					items: items.map((item, index) => ({
						input_index: index,
						file_path: item.filePath,
						job_id: jobId,
					})),
					max_concurrent: 1,
				});
				for (const [index, item] of items.entries()) {
					emitTestEvent('processing-progress', {
						operation_kind: 'metadataSave',
						stage: 'writing',
						percentage: 0,
						message: `Saving metadata ${index + 1}/${items.length}`,
						current_file: item.filePath,
						eta_seconds: null,
						job_id: jobId,
						input_index: index,
					});
					emitTestEvent('processing-progress', {
						operation_kind: 'metadataSave',
						stage: 'completed',
						percentage: 100,
						message: `Saved metadata ${index + 1}/${items.length}`,
						current_file: item.filePath,
						eta_seconds: 0,
						job_id: jobId,
						input_index: index,
					});
				}
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
						status: 'success',
						message: 'Metadata saved',
						error: null,
					})),
				});
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
				});
			}
			case 'cancel_processing': {
				const args = _args as { jobId?: string | null } | undefined;
				emitTestEvent('processing-progress', {
					operation_kind: 'processingBatch',
					stage: 'cancelled',
					percentage: 0,
					message: 'Cancelled by user',
					current_file: '',
					eta_seconds: 0,
					job_id: args?.jobId ?? null,
					input_index: null,
				});
				return Promise.resolve('cancel requested');
			}
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
