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

type TestEventHandler = (event: { event: string; id: number; payload: unknown }) => void;
const eventListeners = new Map<string, Set<TestEventHandler>>();
let mockJobCounter = 0;

function emitTestEvent(event: string, payload: unknown): void {
	const handlers = eventListeners.get(event);
	if (!handlers) return;
	for (const handler of handlers) {
		handler({ event, id: Date.now(), payload });
	}
}

// Mock Tauri's invoke API
vi.mock('@tauri-apps/api/core', () => ({
	invoke: vi.fn().mockImplementation((cmd: string, _args?: unknown) => {
		switch (cmd) {
			case 'analyze_audio_files':
				return Promise.resolve({
					files: [
						{
							path: '/mock/path/chapter1.mp3',
							size: 15 * 1024 * 1024,
							duration: 300,
							isValid: true,
							bitrate: 64,
							sampleRate: 44100,
							channels: 1,
						},
						{
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
				return Promise.resolve([
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
				]);
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
				console.warn(`[Test Mock] Unhandled invoke: ${cmd}`);
				return Promise.resolve(undefined);
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
