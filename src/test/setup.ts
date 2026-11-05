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
		.toSorted((a, b) => b.sequence - a.sequence)
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

function mockOperationSnapshot(operationId: string, inputFiles: string[]): OperationSnapshot {
	return {
		operationId,
		sequence: mockJobCounter,
		revision: 1,
		kind: 'metadataSave',
		status: 'accepted' as const,
		title: `Metadata save (${inputFiles.length} files)`,
		createdAtMs: Date.now(),
		startedAtMs: null,
		finishedAtMs: null,
		cancellable: true,
		cancelRequested: false,
		lanes: ['metadataWrite'],
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
			childJobId: `metadata-${index}`,
			operationId,
			label: pathBasename(path, { fallback: 'path' }),
			status: 'queued' as const,
			startedAtMs: null,
			finishedAtMs: null,
			lane: 'metadataWrite',
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
			warnings: [],
			outputUpdate: null,
		})),
		terminalSummary: null,
		errors: [],
		logTail: [],
	};
}

export function publishMockMetadataSave(filePaths: string[]): void {
	mockJobCounter += 1;
	const operationId = `mock-metadata-operation-${mockJobCounter}`;
	const baseSnapshot = mockOperationSnapshot(operationId, filePaths);
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
			default:
				throw new Error(`[Test Mock] Unhandled Tauri invoke: ${cmd}`);
		}
	}),
	// oxlint-disable-next-line typescript/no-extraneous-class -- the generated bindings import Channel; tests never construct one.
	Channel: class MockChannel {},
}));

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

vi.mock('@tauri-apps/plugin-dialog', () => ({
	open: vi.fn().mockResolvedValue(null),
}));

vi.mock('@tauri-apps/plugin-opener', () => ({
	openUrl: vi.fn().mockResolvedValue(undefined),
}));

Object.defineProperty(window, '__TAURI_INTERNALS__', {
	value: undefined,
	writable: true,
});

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
