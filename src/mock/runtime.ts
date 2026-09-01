import { emit } from '@tauri-apps/api/event';
import type { InvokeArgs } from '@tauri-apps/api/core';
import type {
	AppSettings,
	AppSettingsPatch,
	JobType,
	OperationKind,
	OperationSnapshot,
	WorkProgressStage,
} from '../lib/generated/tauri';
import { EVENTS } from '../types/events';
import { pathBasename } from '../lib/path/basename';
import {
	analyzeFixturePaths,
	audibleProvider,
	defaultMockSettings,
	discoverFixturePaths,
	FIXTURE_AUDIO_PATHS,
	FIXTURE_CHAPTER_1,
	FIXTURE_COVER_PATH,
	FIXTURE_FFMPEG_PATH,
	FIXTURE_INVALID,
	FIXTURE_LIBRARY_DIR,
	FIXTURE_OUTPUT_DIR,
	loggedOutAudibleState,
	MOCK_AUTH_ERROR,
	MOCK_ENCODE_ERROR,
	runtimeCapabilities,
	supportedImportMetadata,
} from './fixtures';

export const MOCK_SCENARIO_IDS = [
	'empty',
	'files-loaded',
	'encoding-in-progress',
	'error',
	'audible-logged-out',
] as const;

export type MockScenarioId = (typeof MOCK_SCENARIO_IDS)[number];

export const MOCK_SCENARIO_LABELS: Record<MockScenarioId, string> = {
	empty: 'Empty',
	'files-loaded': 'Files loaded',
	'encoding-in-progress': 'Encoding in progress',
	error: 'Error',
	'audible-logged-out': 'Audible logged out',
};

type DialogOptions = {
	multiple?: boolean;
	directory?: boolean;
	title?: string;
};

type MockStore = {
	scenario: MockScenarioId;
	settings: AppSettings;
	maxConcurrentJobs: number;
	pendingOpenedPaths: string[];
	operations: OperationSnapshot[];
	failNextEncode: boolean;
	sequence: number;
};

const handlers: Record<string, (payload: Record<string, unknown>) => unknown> = {
	get_app_settings: () => store.settings,
	update_app_settings: (payload) => updateSettings(payload.patch as AppSettingsPatch | undefined),
	reset_app_settings: () => {
		store.settings = defaultMockSettings();
		store.maxConcurrentJobs = 4;
		return store.settings;
	},
	validate_files: () => 'ok',
	read_audio_metadata: (payload) => metadataForPath(stringArg(payload, 'filePath')),
	write_cover_art: () => null,
	load_cover_art_file: () => [255, 216, 255, 224],
	load_cover_art_from_url: () => [255, 216, 255, 224],
	read_audio_cover_thumbnail: () => null,
	validate_metadata_intent_patch: (payload) => ({
		isValid: true,
		metadataPatch: payload.metadataPatch ?? {},
		fieldErrors: [],
	}),
	save_metadata_to_file: () => null,
	save_metadata_batch: (payload) => saveMetadataBatch(payload),
	search_online_metadata: () => ({
		results: [
			{
				source: 'audnexus',
				sourceId: 'mock-dune',
				title: 'Dune',
				authors: ['Frank Herbert'],
				narrators: ['Scott Brick'],
				series: 'Dune',
				seriesPart: '1',
				subseries: null,
				subseriesPart: null,
				description: 'Mock lookup result. No provider was contacted.',
				publishedDate: '1965',
				durationSeconds: 700,
				coverUrl: null,
				audibleOnly: false,
			},
		],
		diagnostics: [],
	}),
	analyze_audio_files: (payload) => analyzeFixturePaths(stringArrayArg(payload, 'filePaths')),
	get_supported_audio_import_metadata: () => supportedImportMetadata(),
	discover_audio_import_paths: (payload) =>
		discoverFixturePaths(stringArrayArg(payload, 'inputPaths')),
	take_opened_audio_files: () => {
		const opened = store.pendingOpenedPaths;
		store.pendingOpenedPaths = [];
		return opened;
	},
	list_remote_source_providers: () => [audibleProvider()],
	get_remote_source_account_state: () => loggedOutAudibleState(),
	start_remote_source_auth: () => ({
		providerId: 'audible',
		authorizationUrl: 'https://mock.invalid/audible-auth',
		handoffPathHint: '/mock/audible-handoff',
		message: 'Mock auth does not open a real Audible login.',
	}),
	complete_remote_source_auth: () => loggedOutAudibleState(),
	logout_remote_source_account: () => loggedOutAudibleState(),
	load_remote_source_library: () => ({
		providerId: 'audible',
		titles: [],
		diagnostics: [
			{
				kind: 'authRequired',
				titleId: null,
				message: 'Mock Audible is logged out. Library was not loaded.',
			},
		],
	}),
	start_remote_source_acquisition: () => {
		throw { ...MOCK_AUTH_ERROR };
	},
	get_remote_source_acquisition_status: () => {
		throw { ...MOCK_AUTH_ERROR };
	},
	cancel_remote_source_acquisition: () => {
		throw { ...MOCK_AUTH_ERROR };
	},
	purge_remote_source_session: () => null,
	validate_encoder_settings: () => 'ok',
	get_runtime_settings_capabilities: () => runtimeCapabilities(),
	preview_output_path: (payload) => previewPath(payload),
	preflight_processing_plan: (payload) => preflightPlan(payload),
	get_max_concurrent_jobs: () => store.maxConcurrentJobs,
	set_max_concurrent_jobs: (payload) => {
		const next = payload.maxConcurrent;
		if (typeof next === 'number' && Number.isFinite(next)) {
			store.maxConcurrentJobs = next;
			store.settings = {
				...store.settings,
				maxConcurrentJobs: { mode: 'fixed', value: next },
			};
		}
		return store.maxConcurrentJobs;
	},
	process_audiobook_files: (payload) => processAudiobook(payload),
	submit_processing_operation: (payload) => submitProcessing(payload),
	list_work_operations: () => ({ operations: store.operations }),
	get_work_operation: (payload) =>
		findOperation(stringArg(payload, 'operationId')) ??
		buildOperation('processingBatch', [], {
			operationId: stringArg(payload, 'operationId') || 'mock-operation',
			status: 'accepted',
		}),
	cancel_work_operation: (payload) => cancelOperation(stringArg(payload, 'operationId')),
	log_frontend: () => null,
};

let store = emptyStore();
let tickerId: ReturnType<typeof setInterval> | undefined;

function emptyStore(): MockStore {
	return {
		scenario: 'empty',
		settings: defaultMockSettings(),
		maxConcurrentJobs: 4,
		pendingOpenedPaths: [],
		operations: [],
		failNextEncode: false,
		sequence: 0,
	};
}

export function getMockScenario(): MockScenarioId {
	return store.scenario;
}

export function resetMockRuntime(): void {
	stopMockTicker();
	store = emptyStore();
}

export function applyScenario(scenario: MockScenarioId): MockScenarioId {
	stopMockTicker();
	store = emptyStore();
	store.scenario = scenario;

	if (scenario === 'files-loaded' || scenario === 'encoding-in-progress') {
		store.pendingOpenedPaths = [...FIXTURE_AUDIO_PATHS];
	}

	if (scenario === 'error') {
		store.pendingOpenedPaths = [...FIXTURE_AUDIO_PATHS, FIXTURE_INVALID];
		store.failNextEncode = true;
		store.operations = [
			buildOperation('processingBatch', [...FIXTURE_AUDIO_PATHS], {
				status: 'failed',
				percentage: 40,
				message: MOCK_ENCODE_ERROR.message,
				stage: 'failed',
			}),
		];
	}

	if (scenario === 'encoding-in-progress') {
		const running = buildOperation('processingBatch', [...FIXTURE_AUDIO_PATHS], {
			status: 'running',
			percentage: 35,
			message: 'Converting mock chapter 1',
			stage: 'converting',
		});
		store.operations = [running];
		startEncodeTicker(running.operationId);
	}

	return scenario;
}

export function handleInvoke(cmd: string, payload?: InvokeArgs): unknown {
	if (cmd === 'plugin:dialog|open') {
		return openCannedDialog(dialogOptions(payload));
	}
	if (cmd === 'plugin:dialog|save') {
		return `${FIXTURE_OUTPUT_DIR}/Dune.m4b`;
	}
	if (
		cmd === 'plugin:dialog|message' ||
		cmd === 'plugin:opener|open_url' ||
		cmd === 'plugin:opener|open_path' ||
		cmd === 'plugin:opener|reveal_item_in_dir'
	) {
		return null;
	}

	const handler = handlers[cmd];
	if (!handler) {
		throw {
			code: 'internal_error',
			category: 'internal',
			message: `Mock runtime has no handler for ${cmd}`,
			detail: null,
		};
	}
	return handler(asRecord(payload));
}

export function openCannedDialog(options: DialogOptions): string | string[] | null {
	if (options.multiple && !options.directory) {
		return [...FIXTURE_AUDIO_PATHS];
	}
	if (options.directory) {
		const title = options.title?.toLowerCase() ?? '';
		return title.includes('output') ? FIXTURE_OUTPUT_DIR : FIXTURE_LIBRARY_DIR;
	}
	const title = options.title?.toLowerCase() ?? '';
	if (title.includes('ffmpeg')) {
		return FIXTURE_FFMPEG_PATH;
	}
	return FIXTURE_COVER_PATH;
}

function processAudiobook(payload: Record<string, unknown>) {
	if (store.failNextEncode) {
		throw { ...MOCK_ENCODE_ERROR };
	}
	const request = processRequest(payload);
	const jobId = nextId('mock-preview');
	emitQueue(request.kind, request.inputFiles);
	emitProgress(request.kind, {
		stage: 'converting',
		percentage: 40,
		message: 'Converting mock preview',
		currentFile: request.inputFiles[0] ?? FIXTURE_CHAPTER_1,
		jobId,
	});
	emitProgress(request.kind, {
		stage: 'completed',
		percentage: 100,
		message: 'Mock preview finished. No files were written.',
		currentFile: request.inputFiles[0] ?? FIXTURE_CHAPTER_1,
		jobId,
	});
	return {
		jobType: request.jobType,
		summary: {
			total: request.inputFiles.length,
			succeeded: request.inputFiles.length,
			skipped: 0,
			cancelled: 0,
			failed: 0,
		},
		terminalClass: 'success',
		results: request.inputFiles.map((_, inputIndex) => ({
			inputIndex,
			status: 'success' as const,
			message: 'Mock preview completed',
			error: null,
			previewFilePath: `${FIXTURE_OUTPUT_DIR}/preview.m4b`,
			previewActualSeconds:
				typeof request.previewSeconds === 'number' ? request.previewSeconds : 30,
			jobId,
		})),
	};
}

function submitProcessing(payload: Record<string, unknown>) {
	if (store.failNextEncode) {
		throw { ...MOCK_ENCODE_ERROR };
	}
	const request = payload.request as Record<string, unknown> | undefined;
	const process = processRequest(request ?? payload);
	const snapshot = buildOperation(process.kind, process.inputFiles, {
		status: 'running',
		percentage: 8,
		message: 'Accepted mock encode.',
		stage: 'analyzing',
	});
	store.operations = [snapshot];
	emitQueue(process.kind, process.inputFiles);
	emitProgress(process.kind, {
		stage: 'analyzing',
		percentage: 8,
		message: snapshot.progress.message,
		currentFile: process.inputFiles[0] ?? null,
		jobId: snapshot.operationId,
	});
	emitWorkSnapshots();
	startEncodeTicker(snapshot.operationId);
	return { operationId: snapshot.operationId, snapshot };
}

function saveMetadataBatch(payload: Record<string, unknown>) {
	const items = arrayArg(payload, 'items');
	const filePaths = items.map((item) => {
		const record = asRecord(item);
		return typeof record.filePath === 'string' ? record.filePath : '';
	});
	const snapshot = buildOperation('metadataSave', filePaths, {
		status: 'completed',
		percentage: 100,
		message: `Completed ${items.length} item(s).`,
		stage: 'complete',
	});
	store.operations = [snapshot];
	emitWorkSnapshots();
	return {
		summary: {
			total: items.length,
			succeeded: items.length,
			skipped: 0,
			cancelled: 0,
			failed: 0,
		},
		results: filePaths.map((filePath, inputIndex) => ({
			inputIndex,
			filePath,
			status: 'success' as const,
			message: 'Metadata saved (mock, no file written)',
			error: null,
		})),
	};
}

function cancelOperation(operationId: string): OperationSnapshot {
	const current =
		findOperation(operationId) ??
		buildOperation('processingBatch', [...FIXTURE_AUDIO_PATHS], { operationId });
	const cancelled: OperationSnapshot = {
		...current,
		status: 'cancelling',
		cancelRequested: true,
		cancellable: false,
		progress: {
			...current.progress,
			stage: 'cancelled',
			message: 'Cancel requested in mock runtime.',
		},
	};
	store.operations = store.operations.map((operation) =>
		operation.operationId === cancelled.operationId ? cancelled : operation,
	);
	stopMockTicker();
	emitWorkSnapshots();
	return cancelled;
}

function updateSettings(patch: AppSettingsPatch | undefined): AppSettings {
	if (!patch) {
		return store.settings;
	}
	store.settings = {
		...store.settings,
		maxConcurrentJobs: patch.maxConcurrentJobs ?? store.settings.maxConcurrentJobs,
		encoderDefaults: patch.encoderDefaults ?? store.settings.encoderDefaults,
		outputDefaults: patch.outputDefaults
			? {
					outputDirectory: patch.outputDefaults.outputDirectory ?? null,
					outputNaming: {
						preset: patch.outputDefaults.outputNaming.preset,
						includeYear: patch.outputDefaults.outputNaming.includeYear,
						customTemplate: patch.outputDefaults.outputNaming.customTemplate ?? null,
					},
				}
			: store.settings.outputDefaults,
		toolchain: patch.toolchain
			? { externalFfmpegPath: patch.toolchain.externalFfmpegPath ?? null }
			: store.settings.toolchain,
		startupBehavior: patch.startupBehavior ?? store.settings.startupBehavior,
		pinnedDefaults: patch.pinnedDefaults ?? store.settings.pinnedDefaults,
	};
	return store.settings;
}

function processRequest(payload: Record<string, unknown>): {
	inputFiles: string[];
	jobType: JobType;
	kind: Extract<OperationKind, 'processingBatch' | 'processingMerge'>;
	previewSeconds: unknown;
} {
	const nested = asRecord(payload.payload);
	const inputFiles = stringArrayArg(nested, 'inputFiles');
	const jobType: JobType = nested.jobType === 'merge' ? 'merge' : 'batch';
	return {
		inputFiles: inputFiles.length > 0 ? inputFiles : [...FIXTURE_AUDIO_PATHS],
		jobType,
		kind: jobType === 'merge' ? 'processingMerge' : 'processingBatch',
		previewSeconds: payload.previewSeconds ?? nested.previewSeconds,
	};
}

function preflightPlan(payload: Record<string, unknown>) {
	const request = processRequest(payload);
	return {
		jobType: request.jobType,
		previewSeconds: typeof request.previewSeconds === 'number' ? request.previewSeconds : null,
		collisionPolicy: 'fail',
		planSignature: 'mock-preflight',
		outputs: request.inputFiles.map((inputPath, inputIndex) => ({
			inputIndex,
			inputPath,
			kind: 'final',
			requestedPath: `${FIXTURE_OUTPUT_DIR}/Frank Herbert/Dune.m4b`,
			resolvedPath: `${FIXTURE_OUTPUT_DIR}/Frank Herbert/Dune.m4b`,
			renameCandidate: null,
			collision: null,
			review: null,
			action: 'write',
		})),
	};
}

function previewPath(payload: Record<string, unknown>): string {
	const outputDir =
		typeof payload.outputDir === 'string' && payload.outputDir.length > 0
			? payload.outputDir
			: FIXTURE_OUTPUT_DIR;
	return `${outputDir}/Frank Herbert/Dune.m4b`;
}

function metadataForPath(filePath: string) {
	const file = analyzeFixturePaths([filePath]).files[0];
	return {
		title: file?.tagTitle ?? null,
		artist: file?.tagArtist ?? null,
		album: file?.tagTitle ?? null,
		composer: 'Scott Brick',
		genre: 'Audiobook',
		date: '1965',
		track: null,
		disk: null,
		comment: null,
		description: 'Mock metadata. No file was read.',
		series: 'Dune',
		series_part: '1',
		subseries: null,
		subseries_part: null,
		album_sort: null,
		cover_art: null,
	};
}

function buildOperation(
	kind: OperationKind,
	inputFiles: readonly string[],
	overrides: {
		operationId?: string;
		status?: OperationSnapshot['status'];
		percentage?: number;
		message?: string;
		stage?: WorkProgressStage;
	} = {},
): OperationSnapshot {
	store.sequence += 1;
	const operationId = overrides.operationId ?? nextId('mock-operation');
	const isMetadata = kind === 'metadataSave';
	const status = overrides.status ?? 'accepted';
	const percentage = overrides.percentage ?? 0;
	const stage = overrides.stage ?? 'pending';
	const title = isMetadata
		? `Metadata save (${inputFiles.length} files)`
		: kind === 'processingMerge'
			? `Merge encode (${inputFiles.length} files)`
			: `Batch encode (${inputFiles.length} files)`;
	const childLane = isMetadata ? 'metadataWrite' : 'encodeCpu';
	const childStatus =
		status === 'failed' ? 'failed' : status === 'completed' ? 'completed' : 'running';
	return {
		operationId,
		sequence: store.sequence,
		kind,
		status,
		title,
		createdAtMs: Date.now(),
		startedAtMs: status === 'accepted' ? null : Date.now(),
		finishedAtMs: status === 'failed' || status === 'completed' ? Date.now() : null,
		cancellable: status === 'accepted' || status === 'running',
		cancelRequested: false,
		lanes: isMetadata ? ['metadataWrite'] : ['analysis', 'encodeCpu', 'outputCommit'],
		sourceInputIds: [],
		progress: {
			stage,
			percentage,
			message: overrides.message ?? 'Accepted.',
			currentItemIndex: 0,
			totalItems: inputFiles.length,
			bytesDownloaded: null,
			bytesTotal: null,
			etaSeconds: status === 'running' ? 45 : null,
		},
		children: inputFiles.map((path, index) => ({
			childJobId: `${isMetadata ? 'metadata' : 'input'}-${index}`,
			operationId,
			label: pathBasename(path, { fallback: 'path' }),
			status: childStatus,
			lane: childLane,
			progress: {
				stage,
				percentage,
				message: overrides.message ?? 'Queued.',
				currentItemIndex: index,
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
			message: status === 'failed' ? MOCK_ENCODE_ERROR.message : null,
		})),
		terminalSummary:
			status === 'failed'
				? {
						total: inputFiles.length,
						succeeded: 0,
						skipped: 0,
						cancelled: 0,
						failed: inputFiles.length,
						message: MOCK_ENCODE_ERROR.message,
					}
				: status === 'completed'
					? {
							total: inputFiles.length,
							succeeded: inputFiles.length,
							skipped: 0,
							cancelled: 0,
							failed: 0,
							message: overrides.message ?? 'Completed.',
						}
					: null,
		warnings: [],
		errors: status === 'failed' ? [MOCK_ENCODE_ERROR.message] : [],
		logTail: [
			{
				timestampMs: Date.now(),
				message: overrides.message ?? title,
				stage,
				childJobId: null,
			},
		],
	};
}

function startEncodeTicker(operationId: string): void {
	stopMockTicker();
	let step = 1;
	tickerId = setInterval(() => {
		const operation = findOperation(operationId);
		if (!operation) {
			stopMockTicker();
			return;
		}
		step += 1;
		const percentage = Math.min(95, 15 + step * 12);
		const stage: WorkProgressStage =
			percentage < 30 ? 'analyzing' : percentage < 70 ? 'converting' : 'writing';
		const next = buildOperation(operation.kind, childPaths(operation), {
			operationId,
			status: 'running',
			percentage,
			stage,
			message: `${stage} mock encode (${percentage}%)`,
		});
		store.operations = [next];
		emitProgress(operation.kind, {
			stage: stage === 'writing' ? 'writing' : stage === 'analyzing' ? 'analyzing' : 'converting',
			percentage,
			message: next.progress.message,
			currentFile: next.children[0]?.sourcePath ?? FIXTURE_CHAPTER_1,
			jobId: operationId,
		});
		emitWorkSnapshots();
		if (percentage >= 95) {
			stopMockTicker();
		}
	}, 400);
}

export function stopMockTicker(): void {
	if (tickerId !== undefined) {
		clearInterval(tickerId);
		tickerId = undefined;
	}
}

function emitProgress(
	kind: OperationKind,
	event: {
		stage: 'analyzing' | 'converting' | 'writing' | 'completed' | 'failed';
		percentage: number;
		message: string;
		currentFile: string | null;
		jobId: string;
	},
): void {
	void emit(EVENTS.PROGRESS, {
		operation_kind: kind,
		stage: event.stage,
		percentage: event.percentage,
		message: event.message,
		current_file: event.currentFile,
		eta_seconds: event.stage === 'completed' ? 0 : 30,
		job_id: event.jobId,
		input_index: 0,
	});
}

function emitQueue(kind: OperationKind, inputFiles: readonly string[]): void {
	void emit(EVENTS.QUEUE, {
		operation_kind: kind,
		items: inputFiles.map((file_path, input_index) => ({ input_index, file_path })),
		max_concurrent: store.maxConcurrentJobs,
	});
}

function emitWorkSnapshots(): void {
	const [snapshot] = store.operations;
	if (snapshot) {
		void emit(EVENTS.WORK_OPERATION_SNAPSHOT, { snapshot });
	}
	void emit(EVENTS.WORK_OPERATION_LIST_SNAPSHOT, { operations: store.operations });
}

function findOperation(operationId: string): OperationSnapshot | undefined {
	return store.operations.find((operation) => operation.operationId === operationId);
}

function childPaths(operation: OperationSnapshot): string[] {
	const paths = operation.children
		.map((child) => child.sourcePath)
		.filter((path): path is string => Boolean(path));
	return paths.length > 0 ? paths : [...FIXTURE_AUDIO_PATHS];
}

function nextId(prefix: string): string {
	store.sequence += 1;
	return `${prefix}-${store.sequence}`;
}

function dialogOptions(payload: InvokeArgs | undefined): DialogOptions {
	const record = asRecord(payload);
	const options = asRecord(record.options);
	return {
		multiple: options.multiple === true,
		directory: options.directory === true,
		title: typeof options.title === 'string' ? options.title : undefined,
	};
}

function asRecord(value: unknown): Record<string, unknown> {
	if (typeof value === 'object' && value !== null && !Array.isArray(value)) {
		return value as Record<string, unknown>;
	}
	return {};
}

function stringArg(payload: Record<string, unknown>, key: string): string {
	const value = payload[key];
	return typeof value === 'string' ? value : '';
}

function arrayArg(payload: Record<string, unknown>, key: string): unknown[] {
	const value = payload[key];
	return Array.isArray(value) ? value : [];
}

function stringArrayArg(payload: Record<string, unknown>, key: string): string[] {
	return arrayArg(payload, key).filter((entry): entry is string => typeof entry === 'string');
}
