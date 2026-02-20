/**
 * Tests for the Tauri tauri client boundary.
 *
 * These tests verify boundary normalization and command/event wiring
 * against mocked Tauri APIs.
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { defaultEncoderSettings } from '../types/audio';

// Note: Tauri APIs are auto-mocked by src/test/setup.ts

describe('tauriClient', () => {
	beforeEach(() => {
		vi.clearAllMocks();
	});

	describe('typed command helpers', () => {
		it('should be importable', async () => {
			const { tauriClient } = await import('./tauri/client');
			expect(tauriClient).toBeDefined();
			expect(typeof tauriClient.analyzeAudioFiles).toBe('function');
			expect(typeof tauriClient.processAudiobookFilesV2).toBe('function');
			expect(typeof tauriClient.cancelProcessing).toBe('function');
		});
	});

	describe('listen', () => {
		it('should be importable', async () => {
			const { tauriClient } = await import('./tauri/client');
			expect(typeof tauriClient.listen).toBe('function');
		});
	});

	describe('open', () => {
		it('should be importable', async () => {
			const { tauriClient } = await import('./tauri/client');
			expect(typeof tauriClient.open).toBe('function');
		});
	});

	describe('openExternal', () => {
		it('should be importable', async () => {
			const { tauriClient } = await import('./tauri/client');
			expect(typeof tauriClient.openExternal).toBe('function');
		});
	});
});

// Example of how to test with custom mock implementations
describe('tauriClient with custom mocks', () => {
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

describe('tauriClient nullish adapters', () => {
	beforeEach(() => {
		vi.resetModules();
		vi.clearAllMocks();
	});

	it('denormalizes metadata fields to nullable generated shape on save', async () => {
		const { invoke } = await import('@tauri-apps/api/core');
		const mockInvoke = vi.mocked(invoke);
		mockInvoke.mockResolvedValueOnce(null);

		const { tauriClient } = await import('./tauri/client');
		await tauriClient.saveMetadataToFile('/books/a.m4b', {
			title: undefined,
			series_part: '1.0',
			cover_art: undefined,
		});

		const lastCall = mockInvoke.mock.calls[mockInvoke.mock.calls.length - 1];
		const [commandName, args] = lastCall as [
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

	it('compiles metadata intent patch on save before denormalization', async () => {
		const { invoke } = await import('@tauri-apps/api/core');
		const mockInvoke = vi.mocked(invoke);
		mockInvoke.mockResolvedValueOnce(null);

		const { tauriClient } = await import('./tauri/client');
		await tauriClient.saveMetadataIntentToFile('/books/a.m4b', {
			title: { op: 'clear' },
			series_part: { op: 'set', value: '2.0' },
			cover_art: { op: 'clear' },
		});

		const lastCall = mockInvoke.mock.calls[mockInvoke.mock.calls.length - 1];
		const [commandName, args] = lastCall as [
			string,
			{ filePath: string; metadata: Record<string, unknown> },
		];
		expect(commandName).toBe('save_metadata_to_file');
		expect(args.filePath).toBe('/books/a.m4b');
		expect(args.metadata.title).toBe('');
		expect(args.metadata.series_part).toBe('2.0');
		expect(args.metadata.cover_art).toEqual([]);
		expect(args.metadata.artist).toBeNull();
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

		const { tauriClient } = await import('./tauri/client');
		const metadata = await tauriClient.readAudioMetadata('/books/a.m4b');
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

		const { tauriClient } = await import('./tauri/client');
		const result = await tauriClient.processAudiobookFilesV2({
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

		const lastCall = mockInvoke.mock.calls[mockInvoke.mock.calls.length - 1];
		const [commandName, args] = lastCall as [
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

	it('compiles metadata intent map for process command payload', async () => {
		const { invoke } = await import('@tauri-apps/api/core');
		const mockInvoke = vi.mocked(invoke);
		mockInvoke.mockResolvedValueOnce({
			message: 'ok',
			previewFilePath: null,
			previewActualSeconds: null,
			jobId: 'job-1',
		});

		const { tauriClient } = await import('./tauri/client');
		await tauriClient.processAudiobookFilesV2({
			payload: {
				inputFiles: ['/books/a.m4b'],
				outputDir: '/tmp/out',
				settings: defaultEncoderSettings(),
				sampleRate: undefined,
				jobType: 'merge',
				outputNaming: undefined,
			},
			metadataIntent: {
				'/books/a.m4b': {
					title: { op: 'clear' },
					artist: { op: 'set', value: 'Author X' },
				},
			},
			previewSeconds: undefined,
		});

		const lastCall = mockInvoke.mock.calls[mockInvoke.mock.calls.length - 1];
		const [, args] = lastCall as [
			string,
			{
				metadata: Record<string, Record<string, unknown>>;
			},
		];
		expect(args.metadata['/books/a.m4b']?.title).toBe('');
		expect(args.metadata['/books/a.m4b']?.artist).toBe('Author X');
		expect(args.metadata['/books/a.m4b']?.series).toBeNull();
	});

	it('normalizes nullish progress-event payload fields from generated listeners', async () => {
		const { listen } = await import('@tauri-apps/api/event');
		const mockListen = vi.mocked(listen);
		mockListen.mockImplementationOnce((async (_event, handler) => {
			(handler as (event: { event: string; id: number; payload: unknown }) => void)({
				event: 'processing-progress',
				id: 1,
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
		}) as typeof listen);

		const { tauriClient } = await import('./tauri/client');
		let received:
			| {
					current_file?: string;
					eta_seconds?: number;
					job_id?: string;
					input_index?: number;
			  }
			| undefined;

		await tauriClient.listen('processing-progress', (event) => {
			received = event.payload as typeof received;
		});

		expect(received).toBeDefined();
		expect(received?.current_file).toBeUndefined();
		expect(received?.eta_seconds).toBeUndefined();
		expect(received?.job_id).toBeUndefined();
		expect(received?.input_index).toBeUndefined();
	});
});
