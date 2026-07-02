import type { FileListInfo } from '../../types/audio';
import type { MetadataSaveBatchResult } from '../../types/metadata';
import type { MetadataIntentPatch } from '../../types/metadataIntent';
import { get } from 'svelte/store';
import {
	Effect,
	type AppLayer,
	type AppEffect,
	makeWorkflowKit,
	runAppEffect,
} from '../../lib/effect/appEffect';
import { tauriClient } from '../../lib/tauri/client';
import { getCurrentFileList, persistPendingMetadataDraftsForCurrentSelection } from '../fileList';
import { resetDirtyState } from '../metadataForm';
import {
	initStatusPanel,
	isStatusPanelProcessing,
	pushStatusPanelTransientStatus,
} from '../statusPanel';
import { clearPendingMetadataForFile, getPendingMetadataIntentEntries } from './state';
import { metadataSaveInProgressStore } from './saveState';

export interface MetadataSaveWorkflowServices {
	getCurrentFileList: typeof getCurrentFileList;
	initStatusPanel: typeof initStatusPanel;
	isStatusPanelProcessing: typeof isStatusPanelProcessing;
	pushStatusPanelTransientStatus: typeof pushStatusPanelTransientStatus;
	isMetadataSaveInProgress: () => boolean;
	setMetadataSaveInProgress: (isInProgress: boolean) => void;
	persistPendingMetadataDraftsForCurrentSelection: typeof persistPendingMetadataDraftsForCurrentSelection;
	getPendingMetadataIntentEntries: typeof getPendingMetadataIntentEntries;
	saveMetadataBatch: typeof tauriClient.saveMetadataBatch;
	clearPendingMetadataForFile: typeof clearPendingMetadataForFile;
	resetDirtyState: typeof resetDirtyState;
	console: Pick<Console, 'error' | 'log'>;
}

export type MetadataSaveWorkflowServicesId = 'MetadataSession/SaveWorkflowServices';
export type MetadataSaveWorkflowLayer = AppLayer<MetadataSaveWorkflowServicesId>;

// #389 spike: the tag/layer/failure/try-wrapper trio comes from one kit while
// the owner keeps its distinct failure identity for catchTag discrimination.
const kit = makeWorkflowKit(
	'MetadataSession/SaveWorkflowServices',
	'MetadataSaveWorkflowFailed',
)<MetadataSaveWorkflowServices>();

export const MetadataSaveWorkflowServicesTag = kit.Tag;

export function makeMetadataSaveWorkflowServicesLayer(
	services: MetadataSaveWorkflowServices,
): MetadataSaveWorkflowLayer {
	return kit.makeLive(services);
}

// Cross-owner functions are wrapped so the import binding is read at CALL
// time, not at module init: this file sits on two static cycles
// (fileList -> metadataStaging -> metadataSession -> here -> fileList, and the
// statusPanel equivalent), and a value capture mid-cycle would freeze
// `undefined` into the live layer.
const liveMetadataSaveWorkflowServices = {
	getCurrentFileList: () => getCurrentFileList(),
	initStatusPanel: () => initStatusPanel(),
	isStatusPanelProcessing: () => isStatusPanelProcessing(),
	pushStatusPanelTransientStatus: (message, options) =>
		pushStatusPanelTransientStatus(message, options),
	isMetadataSaveInProgress: () => get(metadataSaveInProgressStore),
	setMetadataSaveInProgress: (isInProgress) => {
		metadataSaveInProgressStore.set(isInProgress);
	},
	persistPendingMetadataDraftsForCurrentSelection: (options) =>
		persistPendingMetadataDraftsForCurrentSelection(options),
	getPendingMetadataIntentEntries,
	saveMetadataBatch: (requests) => tauriClient.saveMetadataBatch(requests),
	clearPendingMetadataForFile,
	resetDirtyState: () => resetDirtyState(),
	console,
} satisfies MetadataSaveWorkflowServices;

export const MetadataSaveWorkflowLive = makeMetadataSaveWorkflowServicesLayer(
	liveMetadataSaveWorkflowServices,
);

export type MetadataSaveWorkflowEntryServices = Pick<
	MetadataSaveWorkflowServices,
	| 'getCurrentFileList'
	| 'initStatusPanel'
	| 'isStatusPanelProcessing'
	| 'pushStatusPanelTransientStatus'
	| 'isMetadataSaveInProgress'
	| 'setMetadataSaveInProgress'
	| 'console'
>;

export const liveMetadataSaveWorkflowEntryServices = {
	getCurrentFileList: () => getCurrentFileList(),
	initStatusPanel: () => initStatusPanel(),
	isStatusPanelProcessing: () => isStatusPanelProcessing(),
	pushStatusPanelTransientStatus: (message, options) =>
		pushStatusPanelTransientStatus(message, options),
	isMetadataSaveInProgress: () => get(metadataSaveInProgressStore),
	setMetadataSaveInProgress: (isInProgress) => {
		metadataSaveInProgressStore.set(isInProgress);
	},
	console,
} satisfies MetadataSaveWorkflowEntryServices;

export const MetadataSaveWorkflowFailed = kit.Failed;
export type MetadataSaveWorkflowFailed = InstanceType<typeof kit.Failed>;

interface MetadataSaveRunState {
	enteredSave: boolean;
}

export interface PreparedMetadataSaveWorkflowEntry {
	readonly fileList: FileListInfo;
}

const workflowSync = kit.trySync;
const workflowPromise = kit.tryPromise;

function validFilePathSet(fileList: FileListInfo): Set<string> {
	return new Set(fileList.files.filter((file) => file.isValid).map((file) => file.path));
}

function pendingEntriesForLoadedValidFiles(
	services: MetadataSaveWorkflowServices,
	fileList: FileListInfo,
): Array<[string, MetadataIntentPatch]> {
	const validFilePaths = validFilePathSet(fileList);
	return services
		.getPendingMetadataIntentEntries()
		.filter(([filePath]) => validFilePaths.has(filePath));
}

function saveRequestsFromPendingEntries(pendingEntries: Array<[string, MetadataIntentPatch]>) {
	return pendingEntries.map(([filePath, metadataIntent]) => ({
		filePath,
		metadataPatch: metadataIntent,
	}));
}

function handleMetadataSaveResult(
	services: MetadataSaveWorkflowServices,
	result: MetadataSaveBatchResult,
): void {
	for (const entry of result.results) {
		if (entry.status === 'success') {
			services.clearPendingMetadataForFile(entry.filePath);
		} else if (entry.status === 'failed') {
			services.console.error(
				`Failed metadata save for ${entry.filePath}:`,
				entry.error ?? entry.message,
			);
		}
	}

	services.resetDirtyState();
	services.console.log(
		`Metadata save complete: success=${result.summary.succeeded}, failed=${result.summary.failed}, cancelled=${result.summary.cancelled}`,
	);
	// Progress + terminal truth render in the Work Center (the save runs as a
	// WorkRuntime MetadataSave operation); the synchronous result drives only the
	// per-file pending-draft clearing above.
}

function reportWorkflowFailure(
	services: MetadataSaveWorkflowServices,
	error: MetadataSaveWorkflowFailed,
): AppEffect<void> {
	return Effect.sync(() => {
		services.console.error(`Failed to save metadata: ${error.message}`, error.cause);
		services.pushStatusPanelTransientStatus('Save failed - see console', { ttlMs: 3_000 });
	});
}

function resetInProgressWhenNeeded(
	state: MetadataSaveRunState,
): AppEffect<void, never, MetadataSaveWorkflowServicesId> {
	return Effect.gen(function* () {
		if (!state.enteredSave) {
			return;
		}
		const services = yield* MetadataSaveWorkflowServicesTag;
		yield* Effect.sync(() => services.setMetadataSaveInProgress(false));
	});
}

function resolveMetadataSaveEntry(
	services: MetadataSaveWorkflowEntryServices,
): PreparedMetadataSaveWorkflowEntry | null {
	const fileList = services.getCurrentFileList();
	if (!fileList?.files.length) {
		services.console.log('No files loaded - nothing to save');
		return null;
	}

	services.initStatusPanel();
	if (services.isStatusPanelProcessing()) {
		services.console.log('Processing in progress - cannot save metadata now');
		return null;
	}

	services.pushStatusPanelTransientStatus('Preparing metadata save...', { ttlMs: 1_000 });

	if (services.isMetadataSaveInProgress()) {
		services.pushStatusPanelTransientStatus('Save already in progress...', {
			ttlMs: 1_500,
		});
		return null;
	}

	services.setMetadataSaveInProgress(true);
	return { fileList };
}

export function enterMetadataSaveWorkflow(
	services: MetadataSaveWorkflowEntryServices,
): PreparedMetadataSaveWorkflowEntry | null {
	return resolveMetadataSaveEntry(services);
}

function prepareMetadataSaveWorkflowEntry(
	state: MetadataSaveRunState,
	preparedEntry: PreparedMetadataSaveWorkflowEntry | undefined,
): AppEffect<FileListInfo | null, MetadataSaveWorkflowFailed, MetadataSaveWorkflowServicesId> {
	return Effect.gen(function* () {
		if (preparedEntry) {
			state.enteredSave = true;
			return preparedEntry.fileList;
		}

		const services = yield* MetadataSaveWorkflowServicesTag;
		const entry = yield* workflowSync(
			() => resolveMetadataSaveEntry(services),
			'Failed to prepare metadata save entry.',
		);
		if (!entry) {
			return null;
		}
		state.enteredSave = true;
		return entry.fileList;
	});
}

function metadataSaveWorkflowBody(
	state: MetadataSaveRunState,
	preparedEntry?: PreparedMetadataSaveWorkflowEntry,
): AppEffect<void, MetadataSaveWorkflowFailed, MetadataSaveWorkflowServicesId> {
	return Effect.gen(function* () {
		const services = yield* MetadataSaveWorkflowServicesTag;
		const fileList = yield* prepareMetadataSaveWorkflowEntry(state, preparedEntry);
		if (!fileList) {
			return;
		}

		const staged = yield* workflowPromise(
			() => services.persistPendingMetadataDraftsForCurrentSelection({ showStatus: false }),
			'Failed to persist pending metadata drafts.',
		);
		if (!staged) {
			yield* workflowSync(
				() =>
					services.pushStatusPanelTransientStatus('Fix metadata validation errors before saving.', {
						ttlMs: 3_000,
					}),
				'Failed to report metadata validation failure.',
			);
			return;
		}

		const pendingEntries = yield* workflowSync(
			() => pendingEntriesForLoadedValidFiles(services, fileList),
			'Failed to collect pending metadata changes.',
		);
		if (pendingEntries.length === 0) {
			yield* workflowSync(
				() =>
					services.pushStatusPanelTransientStatus('No pending metadata changes', {
						ttlMs: 2_000,
					}),
				'Failed to report metadata save status.',
			);
			return;
		}

		const result = yield* workflowPromise(
			() => services.saveMetadataBatch(saveRequestsFromPendingEntries(pendingEntries)),
			'Failed to save metadata.',
		);
		yield* workflowSync(
			() => handleMetadataSaveResult(services, result),
			'Failed to handle metadata save result.',
		);
	});
}

export function metadataSaveWorkflowExecution(): AppEffect<
	void,
	MetadataSaveWorkflowFailed,
	MetadataSaveWorkflowServicesId
> {
	const state = { enteredSave: false };
	return metadataSaveWorkflowBody(state).pipe(Effect.ensuring(resetInProgressWhenNeeded(state)));
}

export function metadataSaveWorkflowProgram(
	preparedEntry?: PreparedMetadataSaveWorkflowEntry,
): AppEffect<void, never, MetadataSaveWorkflowServicesId> {
	const state = { enteredSave: preparedEntry !== undefined };
	return metadataSaveWorkflowBody(state, preparedEntry).pipe(
		Effect.catchAll((error) =>
			Effect.gen(function* () {
				const services = yield* MetadataSaveWorkflowServicesTag;
				yield* reportWorkflowFailure(services, error);
			}),
		),
		Effect.ensuring(resetInProgressWhenNeeded(state)),
	);
}

export function runMetadataSaveWorkflow(
	layer: MetadataSaveWorkflowLayer,
	preparedEntry?: PreparedMetadataSaveWorkflowEntry,
): Promise<void> {
	return runAppEffect(metadataSaveWorkflowProgram(preparedEntry).pipe(Effect.provide(layer)));
}

/**
 * The one public entry: gate synchronously (so double-invokes are blocked
 * before any await), then run the save workflow on the live layer.
 */
export async function saveMetadataFromUI(): Promise<void> {
	const preparedEntry = enterMetadataSaveWorkflow(liveMetadataSaveWorkflowEntryServices);
	if (!preparedEntry) {
		return;
	}

	try {
		await runMetadataSaveWorkflow(MetadataSaveWorkflowLive, preparedEntry);
	} catch (error) {
		liveMetadataSaveWorkflowEntryServices.setMetadataSaveInProgress(false);
		throw error;
	}
}
