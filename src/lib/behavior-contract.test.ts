import { describe, expect, it } from 'vitest';

import { tauriClient, TAURI_APP_EVENT_NAMES, TAURI_COMMAND_NAMES } from './tauri/client';
import { defaultEncoderSettings, type ProcessPayload } from '../types/audio';
import {
	EVENTS,
	STAGES,
	type ProcessingProgressEvent,
	type ProcessingQueueEvent,
} from '../types/events';

const EXPECTED_COMMAND_NAMES = [
	'analyze_audio_files',
	'cancel_processing',
	'discover_audio_import_paths',
	'echo',
	'get_app_settings',
	'get_max_concurrent_jobs',
	'get_runtime_settings_capabilities',
	'get_supported_audio_import_metadata',
	'list_available_encoders',
	'load_cover_art_file',
	'load_cover_art_from_url',
	'ping',
	'preflight_processing_plan',
	'preview_output_path',
	'process_audiobook_files',
	'read_audio_metadata',
	'refresh_external_toolchain',
	'reset_app_settings',
	'save_metadata_batch',
	'save_metadata_to_file',
	'search_online_metadata',
	'set_max_concurrent_jobs',
	'take_opened_audio_files',
	'update_app_settings',
	'validate_encoder_settings',
	'validate_files',
	'write_cover_art',
] as const;

function sleep(ms: number): Promise<void> {
	return new Promise((resolve) => setTimeout(resolve, ms));
}

async function waitFor(predicate: () => boolean, timeoutMs: number): Promise<void> {
	const started = Date.now();
	while (!predicate()) {
		if (Date.now() - started > timeoutMs) {
			throw new Error(`Timed out after ${timeoutMs}ms waiting for condition`);
		}
		await sleep(25);
	}
}

describe('compatibility guards', () => {
	it('keeps command names stable', () => {
		expect([...TAURI_COMMAND_NAMES].sort()).toEqual([...EXPECTED_COMMAND_NAMES].sort());
	});

	it('keeps app event names stable', () => {
		expect([...TAURI_APP_EVENT_NAMES]).toEqual([
			EVENTS.PROGRESS,
			EVENTS.QUEUE,
			EVENTS.OPENED_AUDIO_FILES,
		]);
	});
});

describe('behavior-first IPC smoke', () => {
	it('preserves analyze outcome contract', async () => {
		const result = await tauriClient.analyzeAudioFiles([
			'/mock/path/chapter1.mp3',
			'/mock/path/chapter2.mp3',
		]);

		expect(result.validCount).toBeGreaterThan(0);
		expect(result.files.length).toBeGreaterThan(0);
		expect(typeof result.totalDuration).toBe('number');
		expect(typeof result.totalSize).toBe('number');
	});

	it('preserves metadata lookup outcome contract', async () => {
		const results = await tauriClient.searchOnlineMetadata({
			query: 'mock search',
			sources: ['audnexus'],
			limit: 8,
		});

		expect(results.length).toBeGreaterThan(0);
		expect(results[0]?.source).toBe('audnexus');
		expect(typeof results[0]?.title).toBe('string');
	});

	it('preserves processing event behavior invariants', async () => {
		const progressEvents: ProcessingProgressEvent[] = [];
		const queueEvents: ProcessingQueueEvent[] = [];

		const unlistenProgress = await tauriClient.listen(EVENTS.PROGRESS, (event) => {
			progressEvents.push(event.payload);
		});
		const unlistenQueue = await tauriClient.listen(EVENTS.QUEUE, (event) => {
			queueEvents.push(event.payload);
		});

		const payload: ProcessPayload = {
			inputFiles: ['/mock/path/chapter1.mp3', '/mock/path/chapter2.mp3'],
			outputDir: '/mock/output',
			settings: defaultEncoderSettings(),
			sampleRate: 'auto',
			jobType: 'batch',
			outputNaming: {
				preset: 'absDefault',
				includeYear: false,
				customTemplate: undefined,
			},
		};

		await tauriClient.processAudiobookFiles({
			payload,
			metadataIntent: null,
			previewSeconds: null,
		});

		await sleep(250);
		await tauriClient.cancelProcessing('mock-job-1');

		await waitFor(
			() =>
				progressEvents.some((e) => e.stage === STAGES.completed || e.stage === STAGES.cancelled),
			4_000,
		);

		expect(queueEvents.length).toBeGreaterThan(0);
		expect(queueEvents[0]?.operation_kind).toBe('processingBatch');
		expect(queueEvents[0]?.items.length).toBeGreaterThan(0);
		expect(queueEvents[0]?.max_concurrent).toBeGreaterThan(0);

		const seenStages = new Set(progressEvents.map((event) => event.stage));
		expect(
			seenStages.has(STAGES.converting) ||
				seenStages.has(STAGES.completed) ||
				seenStages.has(STAGES.cancelled),
		).toBe(true);

		for (const event of progressEvents) {
			expect(event.operation_kind).toBe('processingBatch');
			expect(event.percentage).toBeGreaterThanOrEqual(0);
			expect(event.percentage).toBeLessThanOrEqual(100);
			expect(typeof event.message).toBe('string');
		}

		unlistenProgress();
		unlistenQueue();
	});

	it('preserves cancellation terminal event behavior', async () => {
		const progressEvents: ProcessingProgressEvent[] = [];
		const unlisten = await tauriClient.listen(EVENTS.PROGRESS, (event) => {
			progressEvents.push(event.payload);
		});

		await tauriClient.cancelProcessing('mock-job-1');

		await waitFor(() => progressEvents.some((event) => event.stage === STAGES.cancelled), 2_000);

		expect(progressEvents.some((event) => event.stage === STAGES.cancelled)).toBe(true);
		unlisten();
	});
});
