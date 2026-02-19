/**
 * Tests for the Tauri bridge abstraction.
 *
 * These tests verify that the bridge correctly routes calls
 * to either real Tauri APIs or mocks based on environment.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { defaultEncoderSettings } from '../types/audio';

// Note: Tauri APIs are auto-mocked by src/test/setup.ts

describe('bridge', () => {
	beforeEach(() => {
		vi.clearAllMocks();
	});

	describe('environment detection', () => {
		it('should detect non-Tauri environment in tests', () => {
			// In test environment, __TAURI_INTERNALS__ should be undefined
			expect(
				(window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__,
			).toBeUndefined();
		});
	});

	describe('typed command helpers', () => {
		it('should be importable', async () => {
			// Dynamic import to test module loading
			const { bridge } = await import('./bridge');
			expect(bridge).toBeDefined();
			expect(typeof bridge.analyzeAudioFiles).toBe('function');
			expect(typeof bridge.processAudiobookFilesV2).toBe('function');
			expect(typeof bridge.cancelProcessing).toBe('function');
		});
	});

	describe('listen', () => {
		it('should be importable', async () => {
			const { bridge } = await import('./bridge');
			expect(typeof bridge.listen).toBe('function');
		});
	});

	describe('open', () => {
		it('should be importable', async () => {
			const { bridge } = await import('./bridge');
			expect(typeof bridge.open).toBe('function');
		});
	});

	describe('openExternal', () => {
		it('should be importable', async () => {
			const { bridge } = await import('./bridge');
			expect(typeof bridge.openExternal).toBe('function');
		});
	});
});

// Example of how to test with custom mock implementations
describe('bridge with custom mocks', () => {
	it('example: mock invoke to return specific data', async () => {
		const { invoke } = await import('@tauri-apps/api/core');
		const mockInvoke = vi.mocked(invoke);

		// Set up specific return value for this test
		mockInvoke.mockResolvedValueOnce({ files: [], totalDuration: 0 });

		// Now any code that calls invoke will get this mock
		const result = await invoke('analyze_audio_files', { paths: [] });
		expect(result).toEqual({ files: [], totalDuration: 0 });
	});
});

describe('bridge nullish adapters', () => {
	beforeEach(() => {
		vi.resetModules();
		vi.clearAllMocks();
		(window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {};
	});

	afterEach(() => {
		(window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = undefined;
	});

	it('denormalizes metadata fields to nullable generated shape on save', async () => {
		const { invoke } = await import('@tauri-apps/api/core');
		const mockInvoke = vi.mocked(invoke);
		mockInvoke.mockResolvedValueOnce(null);

		const { bridge } = await import('./bridge');
		await bridge.saveMetadataToFile('/books/a.m4b', {
			title: undefined,
			series_part: '1.0',
			cover_art: undefined,
		});

		const [commandName, args] = mockInvoke.mock.calls.at(-1) as [
			string,
			{ filePath: string; metadata: Record<string, unknown> },
		];
		expect(commandName).toBe('save_metadata_to_file');
		expect(args.filePath).toBe('/books/a.m4b');
		expect(args.metadata.title).toBeNull();
		expect(args.metadata.artist).toBeNull();
		expect(args.metadata.series_part).toBe('1.0');
		expect(args.metadata.cover_art).toBeNull();
	});

	it('normalizes nullable metadata fields from backend responses', async () => {
		const { invoke } = await import('@tauri-apps/api/core');
		const mockInvoke = vi.mocked(invoke);
		mockInvoke.mockResolvedValueOnce({
			title: 'Book A',
			artist: null,
			album: null,
			composer: null,
			genre: null,
			date: null,
			track: null,
			disk: null,
			comment: null,
			description: null,
			series: null,
			series_part: null,
			subseries: null,
			subseries_part: null,
			album_sort: null,
			cover_art: null,
		});

		const { bridge } = await import('./bridge');
		const metadata = await bridge.readAudioMetadata('/books/a.m4b');
		expect(metadata.title).toBe('Book A');
		expect(metadata.artist).toBeUndefined();
		expect(metadata.series).toBeUndefined();
		expect(metadata.cover_art).toBeUndefined();
	});

	it('denormalizes process payload and metadata, then normalizes result nullish fields', async () => {
		const { invoke } = await import('@tauri-apps/api/core');
		const mockInvoke = vi.mocked(invoke);
		mockInvoke.mockResolvedValueOnce({
			message: 'ok',
			previewFilePath: null,
			previewActualSeconds: null,
			jobId: 'job-1',
		});

		const { bridge } = await import('./bridge');
		const result = await bridge.processAudiobookFilesV2({
			payload: {
				inputFiles: ['/books/a.m4b'],
				outputDir: '/tmp/out',
				settings: defaultEncoderSettings(),
				sampleRate: undefined,
				jobType: undefined,
				outputNaming: undefined,
			},
			metadata: {
				'/books/a.m4b': {
					title: undefined,
					cover_art: undefined,
				},
			},
			previewSeconds: undefined,
		});

		const [commandName, args] = mockInvoke.mock.calls.at(-1) as [
			string,
			{
				payload: Record<string, unknown>;
				metadata: Record<string, Record<string, unknown>>;
				previewSeconds: number | null;
			},
		];
		expect(commandName).toBe('process_audiobook_files_v2');
		expect(args.payload.sampleRate).toBeNull();
		expect(args.payload.jobType).toBeNull();
		expect(args.payload.outputNaming).toBeNull();
		expect(args.metadata['/books/a.m4b']?.title).toBeNull();
		expect(args.metadata['/books/a.m4b']?.cover_art).toBeNull();
		expect(args.previewSeconds).toBeNull();
		expect(result.previewFilePath).toBeUndefined();
		expect(result.previewActualSeconds).toBeUndefined();
	});

	it('normalizes nullish progress-event payload fields from generated listeners', async () => {
		const { listen } = await import('@tauri-apps/api/event');
		const mockListen = vi.mocked(listen);
		mockListen.mockImplementationOnce(
			async (_event, handler: (event: { payload: unknown }) => void) => {
				handler({
					payload: {
						stage: 'converting',
						percentage: 42,
						message: 'Working',
						current_file: null,
						eta_seconds: null,
						job_id: null,
						input_index: null,
					},
				});
				return () => {
					/* unlisten */
				};
			},
		);

		const { bridge } = await import('./bridge');
		let received:
			| {
					current_file?: string;
					eta_seconds?: number;
					job_id?: string;
					input_index?: number;
			  }
			| undefined;

		await bridge.listen('processing-progress', (event) => {
			received = event.payload as typeof received;
		});

		expect(received).toBeDefined();
		expect(received?.current_file).toBeUndefined();
		expect(received?.eta_seconds).toBeUndefined();
		expect(received?.job_id).toBeUndefined();
		expect(received?.input_index).toBeUndefined();
	});
});
