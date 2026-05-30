import type { FileListInfo } from '../../types/audio';
import type { MetadataSaveBatchResult } from '../../types/metadata';
import type { MetadataIntentPatch } from '../../types/metadataIntent';
import {
	Data,
	Effect,
	type AppEffect,
	runAppEffect,
	workflowTryPromise,
	workflowTrySync,
} from '../../lib/effect/appEffect';
import {
	MetadataSaveWorkflowServicesTag,
	type MetadataSaveWorkflowLayer,
	type MetadataSaveWorkflowServices,
	type MetadataSaveWorkflowServicesId,
} from './metadataSaveWorkflowServices';

export {
	MetadataSaveWorkflowServicesTag,
	makeMetadataSaveWorkflowServicesLayer,
	type MetadataSaveWorkflowLayer,
	type MetadataSaveWorkflowServices,
	type MetadataSaveWorkflowServicesId,
} from './metadataSaveWorkflowServices';

export class MetadataSaveWorkflowFailed extends Data.TaggedError('MetadataSaveWorkflowFailed')<{
	readonly message: string;
	readonly cause: unknown;
}> {}

interface MetadataSaveRunState {
	enteredSave: boolean;
}

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

export interface PreparedMetadataSaveWorkflowEntry {
	readonly fileList: FileListInfo;
}

function workflowFailure(message: string, cause: unknown): MetadataSaveWorkflowFailed {
	return new MetadataSaveWorkflowFailed({ message, cause });
}

function workflowSync<A>(
	evaluate: () => A,
	message: string,
): AppEffect<A, MetadataSaveWorkflowFailed> {
	return workflowTrySync(evaluate, message, workflowFailure);
}

function workflowPromise<A>(
	evaluate: () => PromiseLike<A>,
	message: string,
): AppEffect<A, MetadataSaveWorkflowFailed> {
	return workflowTryPromise(evaluate, message, workflowFailure);
}

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
	services.completeMetadataSaveInStatusPanel(result);
}

function reportWorkflowFailure(
	services: MetadataSaveWorkflowServices,
	error: MetadataSaveWorkflowFailed,
): AppEffect<void> {
	return Effect.sync(() => {
		services.console.error(`Failed to save metadata: ${error.message}`, error.cause);
		services.failMetadataSaveInStatusPanel('Save failed - see console');
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

		yield* workflowPromise(
			() => services.beginMetadataSaveInStatusPanel(),
			'Failed to begin metadata save status.',
		);
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
