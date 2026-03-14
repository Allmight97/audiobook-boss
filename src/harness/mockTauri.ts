import {
	HARNESS_COVER_ART_BYTES,
	HARNESS_FILE_LIST,
	HARNESS_LOOKUP_RESULTS,
	HARNESS_METADATA_BY_FILE,
	HARNESS_OUTPUT_DIRECTORY,
} from './sampleData';

type TransformCallback = ((payload: unknown) => void) & { __once?: boolean };

type ListenerEntry = {
	callbackId: number;
	eventId: number;
};

type HarnessInvokeArgs = Record<string, unknown>;

type HarnessTauriInternals = {
	invoke: (cmd: string, args?: HarnessInvokeArgs, options?: unknown) => Promise<unknown>;
	transformCallback: (callback: TransformCallback, once?: boolean) => number;
	unregisterCallback: (callbackId: number) => void;
	convertFileSrc: (filePath: string, protocol?: string) => string;
};

type HarnessEventPluginInternals = {
	unregisterListener: (event: string, eventId: number) => void;
};

type WindowWithHarnessTauri = Window & {
	__TAURI_INTERNALS__?: TauriInternals & HarnessTauriInternals;
	__TAURI_EVENT_PLUGIN_INTERNALS__?: HarnessEventPluginInternals;
};

const callbacks = new Map<number, TransformCallback>();
const listeners = new Map<string, ListenerEntry[]>();
let nextCallbackId = 1;
let nextEventId = 1;
let maxConcurrentJobs = 2;

function unregisterListener(event: string, eventId: number): void {
	const entries = listeners.get(event);
	if (!entries) return;
	const remaining = entries.filter((entry) => entry.eventId !== eventId);
	if (remaining.length > 0) {
		listeners.set(event, remaining);
		return;
	}
	listeners.delete(event);
}

function emitHarnessEvent(event: string, payload: unknown): void {
	const entries = listeners.get(event);
	if (!entries) return;

	for (const entry of [...entries]) {
		const callback = callbacks.get(entry.callbackId);
		if (!callback) continue;
		callback({ event, id: entry.eventId, payload });
		if (callback.__once) {
			callbacks.delete(entry.callbackId);
			unregisterListener(event, entry.eventId);
		}
	}
}

function buildPreviewPath(args: HarnessInvokeArgs = {}): string {
	const outputDir =
		typeof args.outputDir === 'string' && args.outputDir.length > 0
			? args.outputDir
			: HARNESS_OUTPUT_DIRECTORY;
	const metadata =
		typeof args.metadata === 'object' && args.metadata !== null
			? (args.metadata as Record<string, unknown>)
			: {};
	const outputNaming =
		typeof args.outputNaming === 'object' && args.outputNaming !== null
			? (args.outputNaming as Record<string, unknown>)
			: {};
	const includeYear = outputNaming.includeYear === true;
	const author =
		typeof metadata.artist === 'string' && metadata.artist.length > 0
			? metadata.artist
			: 'Unknown Author';
	const title =
		typeof metadata.title === 'string' && metadata.title.length > 0 ? metadata.title : 'Untitled';
	const series =
		typeof metadata.series === 'string' && metadata.series.length > 0 ? metadata.series : null;
	const seriesPart =
		typeof metadata.series_part === 'string' && metadata.series_part.length > 0
			? metadata.series_part
			: '1';
	const year =
		typeof metadata.date === 'string' && metadata.date.length >= 4
			? metadata.date.slice(0, 4)
			: '1965';

	if (series) {
		const base = `${outputDir}/${author}/${series}`;
		const leaf = includeYear
			? `Book ${seriesPart} - ${year} - ${title}`
			: `Book ${seriesPart} - ${title}`;
		return `${base}/${leaf}/${leaf}.m4b`;
	}

	const leaf = includeYear ? `${year} - ${title}` : title;
	return `${outputDir}/${author}/${leaf}/${leaf}.m4b`;
}

async function invokeHarnessCommand(cmd: string, args: HarnessInvokeArgs = {}): Promise<unknown> {
	switch (cmd) {
		case 'analyze_audio_files':
			return HARNESS_FILE_LIST;
		case 'read_audio_metadata': {
			const filePath =
				typeof args.filePath === 'string' ? args.filePath : HARNESS_FILE_LIST.files[0]?.path;
			return filePath ? (HARNESS_METADATA_BY_FILE[filePath] ?? {}) : {};
		}
		case 'search_online_metadata':
			return HARNESS_LOOKUP_RESULTS;
		case 'list_available_encoders':
		case 'refresh_external_toolchain':
			return {
				autoEncoder: 'fdk_he_aac',
				fdkAvailable: true,
				fdkSource: 'detected',
				aacAtAvailable: true,
				nativeAacAvailable: true,
				detectedToolchainPath: '/opt/homebrew/bin/ffmpeg',
				overrideToolchainPath: null,
				activeToolchainPath: '/opt/homebrew/bin/ffmpeg',
				overrideInvalid: false,
				overrideError: null,
				statusMessage: 'FDK AAC detected and ready.',
			};
		case 'load_cover_art_from_url':
		case 'load_cover_art_file':
			return HARNESS_COVER_ART_BYTES;
		case 'save_metadata_to_file':
			return null;
		case 'preview_output_path':
			return buildPreviewPath(args);
		case 'process_audiobook_files': {
			const payload =
				typeof args.payload === 'object' && args.payload !== null
					? (args.payload as Record<string, unknown>)
					: {};
			const inputFiles = Array.isArray(payload.inputFiles) ? payload.inputFiles : [];
			const jobType =
				payload.jobType === 'merge' || payload.jobType === 'batch' ? payload.jobType : 'batch';
			const previewSeconds = typeof args.previewSeconds === 'number' ? args.previewSeconds : null;
			const outputDir =
				typeof payload.outputDir === 'string' && payload.outputDir.length > 0
					? payload.outputDir
					: HARNESS_OUTPUT_DIRECTORY;
			const jobId = `harness-job-${Date.now()}`;
			const totalJobs = Math.max(1, inputFiles.length);

			emitHarnessEvent('processing-queue', {
				items: inputFiles.map((filePath, index) => ({
					input_index: index,
					file_path: filePath,
				})),
				max_concurrent: maxConcurrentJobs,
			});

			for (const [index, filePath] of inputFiles.entries()) {
				const perJobId = `${jobId}-${index}`;
				emitHarnessEvent('processing-progress', {
					stage: 'converting',
					percentage: 42,
					message: 'Converting audio',
					current_file: filePath,
					eta_seconds: Math.max(0, (totalJobs - index) * 6),
					job_id: perJobId,
					input_index: index,
				});
				await new Promise((resolve) => window.setTimeout(resolve, 60));
				emitHarnessEvent('processing-progress', {
					stage: 'completed',
					percentage: 100,
					message: 'Processing completed successfully!',
					current_file: filePath,
					eta_seconds: 0,
					job_id: perJobId,
					input_index: index,
				});
			}

			return {
				jobType,
				summary: {
					total: totalJobs,
					succeeded: totalJobs,
					failed: 0,
				},
				results: inputFiles.map((filePath, index) => ({
					inputIndex: index,
					status: 'success',
					message: 'Harness processing completed',
					jobId: `${jobId}-${index}`,
					error: null,
					previewFilePath:
						previewSeconds === null ? null : `${outputDir}/harness-preview-${previewSeconds}.m4b`,
					previewActualSeconds: previewSeconds,
					currentFile: filePath,
				})),
			};
		}
		case 'cancel_processing':
			emitHarnessEvent('processing-progress', {
				stage: 'cancelled',
				percentage: 0,
				message: 'Cancelled by user',
				current_file: '',
				eta_seconds: 0,
				job_id: null,
				input_index: null,
			});
			return 'cancel requested';
		case 'get_max_concurrent_jobs':
			return maxConcurrentJobs;
		case 'set_max_concurrent_jobs': {
			const next =
				typeof args.maxConcurrent === 'number' && Number.isFinite(args.maxConcurrent)
					? Math.max(1, args.maxConcurrent)
					: 2;
			maxConcurrentJobs = next;
			return next;
		}
		case 'plugin:event|listen': {
			const event = typeof args.event === 'string' ? args.event : 'unknown';
			const callbackId = typeof args.handler === 'number' ? args.handler : -1;
			const eventId = nextEventId++;
			const entries = listeners.get(event) ?? [];
			entries.push({ callbackId, eventId });
			listeners.set(event, entries);
			return eventId;
		}
		case 'plugin:event|unlisten': {
			const event = typeof args.event === 'string' ? args.event : 'unknown';
			const eventId = typeof args.eventId === 'number' ? args.eventId : -1;
			unregisterListener(event, eventId);
			return null;
		}
		case 'plugin:event|emit':
		case 'plugin:event|emit_to': {
			const event = typeof args.event === 'string' ? args.event : 'unknown';
			emitHarnessEvent(event, args.payload ?? null);
			return null;
		}
		case 'plugin:dialog|open':
			return args.directory
				? HARNESS_OUTPUT_DIRECTORY
				: HARNESS_FILE_LIST.files.map((file) => file.path);
		case 'plugin:opener|open_path':
		case 'plugin:opener|open_url':
			return null;
		default:
			console.warn(`[harness:mock] unhandled invoke ${cmd}`);
			return null;
	}
}

export function installHarnessTauriMock(): void {
	const harnessWindow = window as WindowWithHarnessTauri;
	if (harnessWindow.__TAURI_INTERNALS__ && harnessWindow.__TAURI_EVENT_PLUGIN_INTERNALS__) {
		return;
	}

	harnessWindow.__TAURI_INTERNALS__ = {
		invoke: (cmd: string, args: HarnessInvokeArgs = {}, _options?: unknown) =>
			invokeHarnessCommand(cmd, args),
		transformCallback: (callback: TransformCallback, once = false) => {
			const id = nextCallbackId++;
			callback.__once = once;
			callbacks.set(id, callback);
			return id;
		},
		unregisterCallback: (callbackId: number) => {
			callbacks.delete(callbackId);
		},
		convertFileSrc: (filePath: string) => filePath,
	};

	harnessWindow.__TAURI_EVENT_PLUGIN_INTERNALS__ = {
		unregisterListener,
	};
}
