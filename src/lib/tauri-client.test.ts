/**
 * Tests for the Tauri tauri client boundary.
 *
 * These tests verify boundary normalization and command/event wiring
 * against mocked Tauri APIs.
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { defaultEncoderSettings, type EncoderSettings } from '../types/audio';
import type { ProcessingProgressEvent } from '../types/events';
import { runtimeSettingsCapabilitiesFixture } from '../test/fixtures/runtimeSettingsCapabilities';

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
			expect(typeof tauriClient.processAudiobookFiles).toBe('function');
			expect(typeof tauriClient.cancelProcessing).toBe('function');
			expect(typeof tauriClient.startRemoteSourceAcquisition).toBe('function');
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

	describe('dialog helpers', () => {
		it('should be importable', async () => {
			const { tauriClient } = await import('./tauri/client');
			expect(typeof tauriClient.openFile).toBe('function');
			expect(typeof tauriClient.openFiles).toBe('function');
			expect(typeof tauriClient.openDirectory).toBe('function');
		});

		it('sets single-file dialog options at the boundary', async () => {
			const { open } = await import('@tauri-apps/plugin-dialog');
			const mockOpen = vi.mocked(open);
			mockOpen.mockResolvedValueOnce('/tmp/book.m4b');

			const { tauriClient } = await import('./tauri/client');
			await expect(tauriClient.openFile({ title: 'Select file' })).resolves.toBe('/tmp/book.m4b');

			expect(mockOpen).toHaveBeenLastCalledWith({
				title: 'Select file',
				multiple: false,
				directory: false,
			});
		});

		it('sets multi-file dialog options at the boundary', async () => {
			const { open } = await import('@tauri-apps/plugin-dialog');
			const mockOpen = vi.mocked(open);
			mockOpen.mockResolvedValueOnce(['/tmp/a.m4b', '/tmp/b.m4b']);

			const { tauriClient } = await import('./tauri/client');
			await expect(tauriClient.openFiles()).resolves.toEqual(['/tmp/a.m4b', '/tmp/b.m4b']);

			expect(mockOpen).toHaveBeenLastCalledWith({ multiple: true, directory: false });
		});

		it('sets directory dialog options at the boundary', async () => {
			const { open } = await import('@tauri-apps/plugin-dialog');
			const mockOpen = vi.mocked(open);
			mockOpen.mockResolvedValueOnce('/tmp/output');

			const { tauriClient } = await import('./tauri/client');
			await expect(tauriClient.openDirectory()).resolves.toBe('/tmp/output');

			expect(mockOpen).toHaveBeenLastCalledWith({ multiple: false, directory: true });
		});
	});

	describe('opener helpers', () => {
		it('should be importable', async () => {
			const { tauriClient } = await import('./tauri/client');
			expect(typeof tauriClient.openPath).toBe('function');
			expect(typeof tauriClient.openUrl).toBe('function');
		});

		it('routes paths and URLs to distinct Tauri opener commands', async () => {
			const { openPath, openUrl } = await import('@tauri-apps/plugin-opener');
			const mockOpenPath = vi.mocked(openPath);
			const mockOpenUrl = vi.mocked(openUrl);

			const { tauriClient } = await import('./tauri/client');
			await tauriClient.openPath('/tmp/preview.m4b');
			await tauriClient.openUrl('https://example.com/login');

			expect(mockOpenPath).toHaveBeenLastCalledWith('/tmp/preview.m4b', undefined);
			expect(mockOpenUrl).toHaveBeenLastCalledWith('https://example.com/login', undefined);
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

	it('compiles metadata intent patch for validation', async () => {
		const { invoke } = await import('@tauri-apps/api/core');
		const mockInvoke = vi.mocked(invoke);
		mockInvoke.mockResolvedValueOnce({
			isValid: false,
			metadataPatch: {
				date: { op: 'set', value: 'not a date' },
			},
			fieldErrors: [
				{
					field: 'date',
					code: 'publication_date_syntax',
					message: 'Publication date must be YYYY or YYYY-MM with month 01-12.',
				},
			],
		});

		const { tauriClient } = await import('./tauri/client');
		const result = await tauriClient.validateMetadataIntentPatch({
			date: { op: 'set', value: 'not a date' },
			title: { op: 'noop' },
		});

		const lastCall = mockInvoke.mock.calls[mockInvoke.mock.calls.length - 1];
		const [commandName, args] = lastCall as [string, { metadataPatch: Record<string, unknown> }];
		expect(commandName).toBe('validate_metadata_intent_patch');
		expect(args.metadataPatch.date).toEqual({ op: 'set', value: 'not a date' });
		expect(args.metadataPatch.title).toBeUndefined();
		expect(result.fieldErrors[0]?.field).toBe('date');
	});

	it('compiles every metadata intent patch in a batch save', async () => {
		const { invoke } = await import('@tauri-apps/api/core');
		const mockInvoke = vi.mocked(invoke);
		mockInvoke.mockResolvedValueOnce({
			summary: {
				total: 2,
				succeeded: 1,
				skipped: 0,
				cancelled: 0,
				failed: 1,
			},
			results: [
				{
					inputIndex: 0,
					filePath: '/books/a.m4b',
					status: 'success',
					message: 'ok',
					error: null,
				},
				{
					inputIndex: 1,
					filePath: '/books/b.m4b',
					status: 'failed',
					message: 'bad',
					error: null,
				},
			],
		});

		const { tauriClient } = await import('./tauri/client');
		const result = await tauriClient.saveMetadataBatch([
			{
				filePath: '/books/a.m4b',
				metadataPatch: { title: { op: 'set', value: 'A' } },
			},
			{
				filePath: '/books/b.m4b',
				metadataPatch: { title: { op: 'clear' }, cover_art: { op: 'clear' } },
			},
		]);

		const lastCall = mockInvoke.mock.calls[mockInvoke.mock.calls.length - 1];
		const [commandName, args] = lastCall as [
			string,
			{
				items: Array<{
					filePath: string;
					metadataPatch: Record<string, unknown>;
				}>;
			},
		];
		expect(commandName).toBe('save_metadata_batch');
		expect(args.items).toHaveLength(2);
		expect(args.items[0]?.metadataPatch.title).toEqual({ op: 'set', value: 'A' });
		expect(args.items[1]?.metadataPatch.title).toEqual({ op: 'clear' });
		expect(args.items[1]?.metadataPatch.cover_art).toEqual({ op: 'clear' });
		expect(result.results[1]?.error).toBeUndefined();
	});

	it('exposes metadata-save helpers without the legacy metadata alias', async () => {
		const { tauriClient } = await import('./tauri/client');
		expect(typeof tauriClient.saveMetadataIntentToFile).toBe('function');
		expect(typeof tauriClient.saveMetadataBatch).toBe('function');
		expect('saveMetadataToFile' in tauriClient).toBe(false);
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
				cancelled: 0,
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
		const result = await tauriClient.processAudiobookFiles({
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
		expect(commandName).toBe('process_audiobook_files');
		expect(args.payload.sampleRate).toBeNull();
		expect(args.payload.jobType).toBeNull();
		expect(args.payload.outputNaming).toBeNull();
		expect(args.metadata['/books/a.m4b']?.title).toEqual({ op: 'clear' });
		expect(args.metadata['/books/a.m4b']?.cover_art).toEqual({ op: 'clear' });
		expect(args.previewSeconds).toBeNull();
		expect(result.jobType).toBe('batch');
		expect(result.summary).toEqual({ total: 1, succeeded: 1, cancelled: 0, failed: 0 });
		expect(result.results).toHaveLength(1);
		expect(result.results[0]?.inputIndex).toBe(0);
		expect(result.results[0]?.status).toBe('success');
		expect(result.results[0]?.previewFilePath).toBeUndefined();
		expect(result.results[0]?.previewActualSeconds).toBeUndefined();
	});

	it('validates encoder settings without external toolchain input', async () => {
		const { invoke } = await import('@tauri-apps/api/core');
		const mockInvoke = vi.mocked(invoke);
		mockInvoke.mockResolvedValueOnce('Encoder settings are valid');

		const { tauriClient } = await import('./tauri/client');
		await tauriClient.validateEncoderSettings(defaultEncoderSettings());

		const lastCall = mockInvoke.mock.calls[mockInvoke.mock.calls.length - 1];
		const [commandName, args] = lastCall as [
			string,
			{
				settings: Record<string, unknown>;
			},
		];
		expect(commandName).toBe('validate_encoder_settings');
		expect(args).toEqual({ settings: defaultEncoderSettings() });
	});

	it('preserves a boundary encoder payload with omitted twoloop through validate_encoder_settings', async () => {
		const { invoke } = await import('@tauri-apps/api/core');
		const mockInvoke = vi.mocked(invoke);
		mockInvoke.mockResolvedValueOnce('Encoder settings are valid');

		const boundarySettings = {
			encoderType: 'native_aac',
			bitrateKbps: 96,
			bitrateMode: { mode: 'cbr' },
			channels: 'stereo',
			afterburner: false,
			threads: { mode: 'auto' },
		} satisfies EncoderSettings;

		const { tauriClient } = await import('./tauri/client');
		await tauriClient.validateEncoderSettings(boundarySettings);

		const lastCall = mockInvoke.mock.calls[mockInvoke.mock.calls.length - 1];
		const [commandName, args] = lastCall as [
			string,
			{
				settings: Record<string, unknown>;
			},
		];
		expect(commandName).toBe('validate_encoder_settings');
		expect(args.settings).toEqual(boundarySettings);
		expect('twoloop' in args.settings).toBe(false);
	});

	it('loads runtime settings capabilities without external toolchain input', async () => {
		const { invoke } = await import('@tauri-apps/api/core');
		const mockInvoke = vi.mocked(invoke);
		mockInvoke.mockResolvedValueOnce(runtimeSettingsCapabilitiesFixture());

		const { tauriClient } = await import('./tauri/client');
		const capabilities = await tauriClient.getRuntimeSettingsCapabilities();

		const lastCall = mockInvoke.mock.calls[mockInvoke.mock.calls.length - 1];
		const [commandName, args = {}] = lastCall as [string, Record<string, unknown>?];
		expect(commandName).toBe('get_runtime_settings_capabilities');
		expect(args).toEqual({});
		expect(capabilities.encoder.bitrateKbpsOptions).toContain(128);
		expect(capabilities.maxConcurrentJobs.fixedOptions).toContain(8);
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
		await tauriClient.processAudiobookFiles({
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

	it('routes remote source acquisition through provider-neutral command payloads', async () => {
		const { invoke } = await import('@tauri-apps/api/core');
		const mockInvoke = vi.mocked(invoke);
		mockInvoke.mockResolvedValueOnce({
			jobId: 'remote-job-1',
			providerId: 'audible',
			status: 'acquiring',
			progress: {
				stage: 'download',
				percentage: 35,
				message: 'Downloading audiobook.',
				bytesDownloaded: 50,
				bytesTotal: 100,
				terminal: false,
			},
			materializedFiles: [],
			supplementalAssets: [],
			diagnostics: [],
		});

		const { tauriClient } = await import('./tauri/client');
		const result = await tauriClient.startRemoteSourceAcquisition({
			providerId: 'audible',
			selections: [{ titleId: 'B000000001', includeSupplementalPdf: true }],
		});

		const lastCall = mockInvoke.mock.calls[mockInvoke.mock.calls.length - 1];
		const [commandName, args] = lastCall as [
			string,
			{ plan: { providerId: string; selections: Array<Record<string, unknown>> } },
		];
		expect(commandName).toBe('start_remote_source_acquisition');
		expect(args.plan).toEqual({
			providerId: 'audible',
			selections: [{ titleId: 'B000000001', includeSupplementalPdf: true }],
		});
		expect(result.jobId).toBe('remote-job-1');
	});

	it('preserves failed process result status, error, and input index from backend results', async () => {
		const { invoke } = await import('@tauri-apps/api/core');
		const mockInvoke = vi.mocked(invoke);
		mockInvoke.mockResolvedValueOnce({
			jobType: 'batch',
			summary: {
				total: 2,
				succeeded: 1,
				cancelled: 0,
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
					error: {
						code: 'decoder_unavailable',
						category: 'toolchain',
						message: 'decoder unavailable',
						detail: 'ffmpeg missing',
					},
					previewFilePath: null,
					previewActualSeconds: null,
				},
			],
		});

		const { tauriClient } = await import('./tauri/client');
		const result = await tauriClient.processAudiobookFiles({
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

		expect(result.summary).toEqual({ total: 2, succeeded: 1, cancelled: 0, failed: 1 });
		expect(result.results[1]).toEqual({
			inputIndex: 1,
			status: 'failed',
			message: 'failed',
			error: {
				code: 'decoder_unavailable',
				category: 'toolchain',
				message: 'decoder unavailable',
				detail: 'ffmpeg missing',
			},
			jobId: undefined,
			previewFilePath: undefined,
			previewActualSeconds: undefined,
		});
	});

	it('unwraps generated Result error responses into normalized app errors', async () => {
		const { invoke } = await import('@tauri-apps/api/core');
		const mockInvoke = vi.mocked(invoke);
		mockInvoke.mockRejectedValueOnce({
			code: 'cancelled',
			category: 'cancellation',
			message: 'Processing was cancelled.',
			detail: 'user requested stop',
		});

		const { tauriClient } = await import('./tauri/client');

		await expect(tauriClient.cancelProcessing('mock-job-1')).rejects.toMatchObject({
			code: 'cancelled',
			category: 'cancellation',
			message: 'Processing was cancelled.',
			detail: 'user requested stop',
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
					operation_kind: 'processingBatch',
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
		let received: ProcessingProgressEvent | undefined;

		await tauriClient.listen('processing-progress', (event) => {
			received = event.payload;
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

	it('loads runtime settings capabilities through the Tauri boundary', async () => {
		const { invoke } = await import('@tauri-apps/api/core');
		const mockInvoke = vi.mocked(invoke);
		mockInvoke.mockResolvedValueOnce({
			encoder: {
				availability: {
					fdkAvailable: true,
					fdkSource: 'detected',
					aacAtAvailable: true,
					nativeAacAvailable: true,
					autoEncoder: 'fdk_he_aac',
					detectedToolchainPath: '/opt/homebrew/bin/ffmpeg',
					statusMessage: 'FDK AAC detected and ready.',
				},
				encoderTypes: ['auto', 'fdk_he_aac', 'aac_at', 'native_aac'],
				autoResolutionOrder: ['fdk_he_aac', 'aac_at', 'native_aac'],
				bitrateKbpsOptions: [64],
				bitrateModesByEncoder: [
					{ encoderType: 'auto', allowedModes: ['vbr'], defaultMode: { mode: 'vbr', value: 3 } },
				],
				vbrLevelMin: 1,
				vbrLevelMax: 5,
				vbrLevelDefault: 3,
				threadFixedMin: 1,
				threadFixedMax: 1024,
				sampleRateAuto: true,
				explicitSampleRates: [44100],
				channelOptions: ['auto', 'mono', 'stereo'],
			},
			maxConcurrentJobs: {
				allowAuto: true,
				autoEffective: 4,
				fixedMin: 1,
				fixedMax: 8,
				fixedOptions: [1, 2, 3, 4, 5, 6, 7, 8],
			},
		});

		const { tauriClient } = await import('./tauri/client');
		const capabilities = await tauriClient.getRuntimeSettingsCapabilities();

		const [commandName, args = {}] = mockInvoke.mock.calls[mockInvoke.mock.calls.length - 1] as [
			string,
			Record<string, unknown>?,
		];
		expect(commandName).toBe('get_runtime_settings_capabilities');
		expect(args).toEqual({});
		expect(capabilities.encoder.availability.detectedToolchainPath).toBe(
			'/opt/homebrew/bin/ffmpeg',
		);
		expect(capabilities.maxConcurrentJobs.fixedOptions).toContain(8);
	});
});

describe('unwrapGeneratedResult', () => {
	it('returns .data on canonical specta success shape', async () => {
		const { unwrapGeneratedResult } = await import('./tauri/appError');
		const result = unwrapGeneratedResult<{ hello: string }>({
			status: 'ok',
			data: { hello: 'world' },
		});
		expect(result).toEqual({ hello: 'world' });
	});

	it('throws normalized AppError on canonical specta error shape', async () => {
		const { unwrapGeneratedResult } = await import('./tauri/appError');
		expect(() =>
			unwrapGeneratedResult({
				status: 'error',
				error: {
					code: 'toolchain_missing',
					category: 'toolchain',
					message: 'ffmpeg not found',
				},
			}),
		).toThrow(
			expect.objectContaining({
				code: 'toolchain_missing',
				category: 'toolchain',
				message: 'ffmpeg not found',
			}) as unknown as Error,
		);
	});

	it('passes through bare scalar values unchanged (get_max_concurrent_jobs path)', async () => {
		const { unwrapGeneratedResult } = await import('./tauri/appError');
		expect(unwrapGeneratedResult<number>(42)).toBe(42);
	});

	it('passes through bare object values unchanged (EncoderAvailability path)', async () => {
		const { unwrapGeneratedResult } = await import('./tauri/appError');
		const bareEncoderAvailability = {
			ffmpegAvailable: true,
			ffprobeAvailable: true,
			availableEncoders: ['aac', 'libmp3lame'],
		};
		const result = unwrapGeneratedResult<typeof bareEncoderAvailability>(bareEncoderAvailability);
		expect(result).toBe(bareEncoderAvailability);
	});

	// Misclassification guard: without the discriminant check on `status === 'ok' | 'error'`,
	// a hypothetical future domain type with an unrelated `status` field (e.g. job state)
	// would be incorrectly stripped to its `.data`. This locks the safety property in place.
	it('passes through records with unrelated status values rather than treating them as Result', async () => {
		const { unwrapGeneratedResult } = await import('./tauri/appError');
		const bareDomainObject = { status: 'pending', data: { jobId: 'abc' } };
		const result = unwrapGeneratedResult<typeof bareDomainObject>(bareDomainObject);
		expect(result).toBe(bareDomainObject);
		expect(result.status).toBe('pending');
	});
});
