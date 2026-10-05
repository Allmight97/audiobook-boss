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
import { audioFile, fakeEngine, resetFakeEngine } from './fixtures/fakeEngine';
import type {
	OperationSnapshot,
	SupportedAudioImportMetadata,
	WorkOperationsSnapshot,
} from '../lib/generated/tauri';

type TestEventHandler = (event: { event: string; id: number; payload: unknown }) => void;
const eventListeners = new Map<string, Set<TestEventHandler>>();
let mockJobCounter = 0;
let mockRevision = 0;
const mockOperations = new Map<string, OperationSnapshot>();

function emitTestEvent(event: string, payload: unknown): void {
	const handlers = eventListeners.get(event);
	if (!handlers) return;
	for (const handler of handlers) {
		handler({ event, id: Date.now(), payload });
	}
}

function mockOrder(): string[] {
	return [...mockOperations.values()]
		.sort((a, b) => b.sequence - a.sequence)
		.map((operation) => operation.operationId);
}

function mockOperationList(): WorkOperationsSnapshot {
	return {
		revision: mockRevision,
		order: mockOrder(),
		operations: mockOrder().map((id) => mockOperations.get(id)!),
	};
}

function publishMockOperation(snapshot: OperationSnapshot): void {
	mockOperations.set(snapshot.operationId, snapshot);
	emitTestEvent('work-operations-update', {
		revision: ++mockRevision,
		order: mockOrder(),
		changed: snapshot,
	});
}

function mockOperationSnapshot(
	operationId: string,
	kind: 'processingBatch' | 'metadataSave',
	inputFiles: string[],
	submittedTitle?: string,
): OperationSnapshot {
	const isMetadataSave = kind === 'metadataSave';
	const title = isMetadataSave
		? `Metadata save (${inputFiles.length} files)`
		: (submittedTitle ?? `Batch encode (${inputFiles.length} files)`);
	const lanes = (
		isMetadataSave ? ['metadataWrite'] : ['analysis', 'encodeCpu', 'outputCommit']
	) as OperationSnapshot['lanes'];
	const childLane = (
		isMetadataSave ? 'metadataWrite' : 'encodeCpu'
	) as OperationSnapshot['lanes'][number];
	return {
		operationId,
		sequence: mockJobCounter,
		revision: 1,
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
			startedAtMs: null,
			finishedAtMs: null,
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
			sourceInputIds: [],
			jobId: null,
			cancellable: false,
			cancelRequested: false,
			message: null,
			outputPath: null,
			supplementalWarning: null,
			outputUpdate: null,
		})),
		terminalSummary: null,
		errors: [],
		logTail: [],
	};
}

/**
 * Publishes what the engine publishes when a metadata save runs: an accepted
 * operation, then running, then completed, one child per file.
 */
export function publishMockMetadataSave(filePaths: string[]): void {
	mockJobCounter += 1;
	const operationId = `mock-metadata-operation-${mockJobCounter}`;
	const baseSnapshot = mockOperationSnapshot(operationId, 'metadataSave', filePaths);
	publishMockOperation(baseSnapshot);
	publishMockOperation({
		...baseSnapshot,
		revision: baseSnapshot.revision + 1,
		status: 'running',
		startedAtMs: Date.now(),
	});
	publishMockOperation({
		...baseSnapshot,
		revision: baseSnapshot.revision + 2,
		status: 'completed' as const,
		startedAtMs: Date.now(),
		finishedAtMs: Date.now(),
		cancellable: false,
		progress: {
			...baseSnapshot.progress,
			stage: 'complete' as const,
			percentage: 100,
			message: `Completed ${filePaths.length} item(s).`,
		},
		children: baseSnapshot.children.map((child) => ({
			...child,
			status: 'completed' as const,
			progress: { ...child.progress, stage: 'complete' as const, percentage: 100 },
			message: 'Metadata saved',
		})),
		terminalSummary: {
			total: filePaths.length,
			succeeded: filePaths.length,
			skipped: 0,
			cancelled: 0,
			failed: 0,
			message: `Completed ${filePaths.length} item(s).`,
		},
	});
}

// Mock Tauri's invoke API
vi.mock('@tauri-apps/api/core', () => ({
	convertFileSrc: (path: string, protocol: string) =>
		`${protocol}://localhost/${encodeURIComponent(path)}`,
	invoke: vi.fn().mockImplementation((cmd: string, _args?: unknown) => {
		const engineArgs = _args as {
			client: number;
			sequence: number;
			intent: never;
			filePaths: string[];
		};
		switch (cmd) {
			// The session and settings are answered by this test's fake engine.
			case 'attach_frontend':
				return fakeEngine().attach();
			case 'session_dispatch':
				return fakeEngine().sessionDispatch(
					engineArgs.client,
					engineArgs.sequence,
					engineArgs.intent,
				);
			case 'settings_dispatch':
				return fakeEngine().settingsDispatch(
					engineArgs.client,
					engineArgs.sequence,
					engineArgs.intent,
				);
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
			case 'list_work_operations':
				return Promise.resolve(mockOperationList());
			case 'cancel_work_operation': {
				const args = _args as { operationId?: string; childJobId?: string | null } | undefined;
				const current = mockOperations.get(args?.operationId ?? '');
				if (!current) return Promise.reject(new Error('Operation not found'));
				if (args?.childJobId) {
					const snapshot: OperationSnapshot = {
						...current,
						revision: current.revision + 1,
						children: current.children.map((child) =>
							child.childJobId === args.childJobId
								? { ...child, cancelRequested: true, cancellable: false }
								: child,
						),
					};
					publishMockOperation(snapshot);
					return Promise.resolve(snapshot);
				}
				if (!current.cancellable) return Promise.resolve(current);
				const snapshot: OperationSnapshot = {
					...current,
					revision: current.revision + 1,
					status: 'cancelling' as const,
					cancelRequested: true,
					cancellable: false,
				};
				publishMockOperation(snapshot);
				return Promise.resolve(snapshot);
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

/** A fresh fake engine whose own changes reach the frontend as events. */
function startFakeEngine(): void {
	resetFakeEngine();
	const engine = fakeEngine();
	engine.analyze = () => [
		audioFile('/mock/path/chapter1.mp3', {
			inputId: 'mock-input-1',
			size: 15 * 1024 * 1024,
			duration: 300,
			bitrate: 64,
			sampleRate: 44100,
			channels: 1,
			format: 'mp3',
			codecLabel: 'MP3',
			selectedDecoder: 'ffmpeg',
		}),
		audioFile('/mock/path/chapter2.mp3', {
			inputId: 'mock-input-2',
			size: 20 * 1024 * 1024,
			duration: 400,
			bitrate: 64,
			sampleRate: 44100,
			channels: 1,
			format: 'mp3',
			codecLabel: 'MP3',
			selectedDecoder: 'ffmpeg',
		}),
	];
	void engine.listenSessionUpdates((update) => emitTestEvent('session-update', update));
}

startFakeEngine();

beforeEach(() => {
	eventListeners.clear();
	mockJobCounter = 0;
	mockRevision = 0;
	mockOperations.clear();
	startFakeEngine();
});

afterEach(() => {
	eventListeners.clear();
	mockJobCounter = 0;
	mockRevision = 0;
	mockOperations.clear();
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
