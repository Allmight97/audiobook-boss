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
			title: { op: 'clear' },
			series_part: { op: 'set', value: '1.0' },
		});

		const lastCall = mockInvoke.mock.calls[mockInvoke.mock.calls.length - 1];
		const [commandName, args] = lastCall as [
			string,
			{ filePath: string; metadataPatch: Record<string, unknown> },
		];
		expect(commandName).toBe('save_metadata_to_file');
		expect(args.filePath).toBe('/books/a.m4b');
		expect(args.metadataPatch.title).toEqual({ op: 'clear' });
		expect(args.metadataPatch.series_part).toEqual({ op: 'set', value: '1.0' });
		expect(args.metadataPatch.artist).toBeUndefined();
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
			{ filePath: string; metadataPatch: Record<string, unknown> },
		];
		expect(commandName).toBe('save_metadata_to_file');
		expect(args.filePath).toBe('/books/a.m4b');
		expect(args.metadataPatch.title).toEqual({ op: 'clear' });
		expect(args.metadataPatch.series_part).toEqual({ op: 'set', value: '2.0' });
		expect(args.metadataPatch.cover_art).toEqual({ op: 'clear' });
		expect(args.metadataPatch.artist).toBeUndefined();
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

	it('denormalizes process payload and compiles metadata patch map, then normalizes result nullish fields', async () => {
		const { invoke } = await import('@tauri-apps/api/core');
		const mockInvoke = vi.mocked(invoke);
		mockInvoke.mockResolvedValueOnce({
			jobType: 'batch',
			summary: {
				total: 1,
				succeeded: 1,
				failed: 0,
			},
			results: [
				{
					inputIndex: 0,
					status: 'success',
					message: 'ok',
					jobId: 'job-1',
					error: null,
					previewFilePath: null,
					previewActualSeconds: null,
				},
			],
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
			metadataIntent: {
				'/books/a.m4b': {
					title: { op: 'clear' },
					cover_art: { op: 'clear' },
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
		expect(args.metadata['/books/a.m4b']?.title).toEqual({ op: 'clear' });
		expect(args.metadata['/books/a.m4b']?.cover_art).toEqual({ op: 'clear' });
		expect(args.previewSeconds).toBeNull();
		expect(result.jobType).toBe('batch');
		expect(result.summary).toEqual({ total: 1, succeeded: 1, failed: 0 });
		expect(result.results).toHaveLength(1);
		expect(result.results[0]?.inputIndex).toBe(0);
		expect(result.results[0]?.status).toBe('success');
		expect(result.results[0]?.previewFilePath).toBeUndefined();
		expect(result.results[0]?.previewActualSeconds).toBeUndefined();
	});

	it('compiles metadata intent map for process command payload', async () => {
		const { invoke } = await import('@tauri-apps/api/core');
		const mockInvoke = vi.mocked(invoke);
		mockInvoke.mockResolvedValueOnce({
			jobType: 'merge',
			summary: {
				total: 1,
				succeeded: 0,
				failed: 1,
			},
			results: [
				{
					inputIndex: null,
					status: 'failed',
					message: 'ok',
					jobId: null,
					error: 'bad output path',
					previewFilePath: null,
					previewActualSeconds: null,
				},
			],
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
		expect(args.metadata['/books/a.m4b']?.title).toEqual({ op: 'clear' });
		expect(args.metadata['/books/a.m4b']?.artist).toEqual({ op: 'set', value: 'Author X' });
		expect(args.metadata['/books/a.m4b']?.series).toBeUndefined();
	});

	it('preserves failed process result status, error, and input index from backend results', async () => {
		const { invoke } = await import('@tauri-apps/api/core');
		const mockInvoke = vi.mocked(invoke);
		mockInvoke.mockResolvedValueOnce({
			jobType: 'batch',
			summary: {
				total: 2,
				succeeded: 1,
				failed: 1,
			},
			results: [
				{
					inputIndex: 0,
					status: 'success',
					message: 'ok',
					jobId: 'job-1',
					error: null,
					previewFilePath: null,
					previewActualSeconds: null,
				},
				{
					inputIndex: 1,
					status: 'failed',
					message: 'failed',
					jobId: null,
					error: 'decoder unavailable',
					previewFilePath: null,
					previewActualSeconds: null,
				},
			],
		});

		const { tauriClient } = await import('./tauri/client');
		const result = await tauriClient.processAudiobookFilesV2({
			payload: {
				inputFiles: ['/books/a.m4b', '/books/b.m4b'],
				outputDir: '/tmp/out',
				settings: defaultEncoderSettings(),
				sampleRate: undefined,
				jobType: 'batch',
				outputNaming: undefined,
			},
			metadataIntent: null,
			previewSeconds: undefined,
		});

		expect(result.summary).toEqual({ total: 2, succeeded: 1, failed: 1 });
		expect(result.results[1]).toEqual({
			inputIndex: 1,
			status: 'failed',
			message: 'failed',
			error: 'decoder unavailable',
			jobId: undefined,
			previewFilePath: undefined,
			previewActualSeconds: undefined,
		});
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

	it('denormalizes preview output naming nullish fields for preview_output_path command', async () => {
		const { invoke } = await import('@tauri-apps/api/core');
		const mockInvoke = vi.mocked(invoke);
		mockInvoke.mockResolvedValueOnce('/tmp/out/Frank Herbert/Dune.m4b');

		const { tauriClient } = await import('./tauri/client');
		const preview = await tauriClient.previewOutputPath({
			outputDir: '/tmp/out',
			metadata: { title: 'Dune', artist: 'Frank Herbert' },
			outputNaming: {
				preset: 'customTemplate',
				includeYear: false,
				customTemplate: undefined,
			},
			sourcePath: '/books/ch01.mp3',
		});

		const lastCall = mockInvoke.mock.calls[mockInvoke.mock.calls.length - 1];
		const [commandName, args] = lastCall as [
			string,
			{
				outputDir: string;
				metadata: Record<string, unknown>;
				outputNaming: Record<string, unknown>;
				sourcePath: string | null;
			},
		];

		expect(commandName).toBe('preview_output_path');
		expect(args.outputDir).toBe('/tmp/out');
		expect(args.metadata.title).toBe('Dune');
		expect(args.outputNaming.preset).toBe('customTemplate');
		expect(args.outputNaming.includeYear).toBe(false);
		expect(args.outputNaming.customTemplate).toBeNull();
		expect(args.sourcePath).toBe('/books/ch01.mp3');
		expect(preview).toBe('/tmp/out/Frank Herbert/Dune.m4b');
	});
});
