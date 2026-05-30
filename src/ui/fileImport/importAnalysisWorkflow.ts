import {
	Data,
	Effect,
	type AppEffect,
	runAppEffect,
	workflowTryPromise,
} from '../../lib/effect/appEffect';
import type { AudioFile, FileListInfo } from '../../types/audio';
import { ImportAnalysisWorkflowLive } from './importAnalysisWorkflowLive';
import {
	duplicateOnlyImportMessage,
	importOrderLockedMessage,
	reportAnalysisFailure,
	reportImportDiscoveryFailure,
	reportImportMetadataFailure,
	reportMetadataStagingFailure,
	reportOpenFileDialogFailure,
	reportOpenFolderDialogFailure,
	unsupportedImportMessage,
} from './importAnalysisWorkflowFeedback';
import {
	ImportAnalysisWorkflowServicesTag,
	type ImportAnalysisWorkflowAction,
	type ImportAnalysisWorkflowLayer,
	type ImportAnalysisWorkflowServices,
	type ImportAnalysisWorkflowServicesId,
} from './importAnalysisWorkflowServices';

export { importOrderLockedMessage } from './importAnalysisWorkflowFeedback';

export {
	ImportAnalysisWorkflowServicesTag,
	makeImportAnalysisWorkflowServicesLayer,
	type ImportAnalysisWorkflowAction,
	type ImportAnalysisWorkflowLayer,
	type ImportAnalysisWorkflowServices,
	type ImportAnalysisWorkflowServicesId,
} from './importAnalysisWorkflowServices';

export class ImportAnalysisWorkflowFailed extends Data.TaggedError('ImportAnalysisWorkflowFailed')<{
	readonly message: string;
	readonly cause: unknown;
}> {}

export type PreparedImportAnalysisWorkflowEntry =
	| {
			readonly type: 'openFiles';
			readonly selectedPaths: ReturnType<ImportAnalysisWorkflowServices['openFiles']>;
			readonly existingFiles: AudioFile[];
	  }
	| {
			readonly type: 'openDirectory';
			readonly selectedPath: ReturnType<ImportAnalysisWorkflowServices['openDirectory']>;
			readonly existingFiles: AudioFile[];
	  }
	| {
			readonly type: 'discoverPaths';
			readonly discoveredPaths: ReturnType<
				ImportAnalysisWorkflowServices['discoverAudioImportPaths']
			>;
			readonly existingFiles: AudioFile[];
	  }
	| {
			readonly type: 'analyzeFiles';
			readonly fileListInfo: ReturnType<ImportAnalysisWorkflowServices['analyzeAudioFiles']>;
			readonly existingFiles: AudioFile[];
	  };

function workflowFailure(message: string, cause: unknown): ImportAnalysisWorkflowFailed {
	return new ImportAnalysisWorkflowFailed({ message, cause });
}

function workflowPromise<A>(
	evaluate: () => PromiseLike<A>,
	message: string,
): AppEffect<A, ImportAnalysisWorkflowFailed> {
	return workflowTryPromise(evaluate, message, workflowFailure);
}

function appendAnalyzedFiles(
	services: ImportAnalysisWorkflowServices,
	fileListInfo: FileListInfo,
	existingFiles: AudioFile[],
): ReturnType<ImportAnalysisWorkflowServices['appendFileList']>['outcome'] {
	const appendResult = services.appendFileList(fileListInfo, {
		existingFiles,
		showDuplicateStatus: false,
	});
	if (appendResult.outcome === 'duplicateOnly') {
		const message = duplicateOnlyImportMessage();
		services.setFileImportError(message);
		services.pushStatusPanelTransientStatus(message, { ttlMs: 2000 });
	}
	return appendResult.outcome;
}

function analyzeFileListInfo(
	services: ImportAnalysisWorkflowServices,
	evaluate: () => ReturnType<ImportAnalysisWorkflowServices['analyzeAudioFiles']>,
): AppEffect<FileListInfo | null, never> {
	return workflowPromise(evaluate, 'Failed to analyze files.').pipe(
		Effect.catchAll((error) =>
			Effect.sync(() => {
				return reportAnalysisFailure(services, error.cause);
			}),
		),
	);
}

function stagePendingMetadataDrafts(
	services: ImportAnalysisWorkflowServices,
): AppEffect<boolean, never> {
	return workflowPromise(
		() => services.persistPendingMetadataDraftsForCurrentSelection(),
		'Failed to stage metadata drafts before import.',
	).pipe(
		Effect.catchAll((error) =>
			Effect.sync(() => {
				return reportMetadataStagingFailure(services, error.cause);
			}),
		),
	);
}

function stageAndAppendAnalyzedFiles(
	services: ImportAnalysisWorkflowServices,
	fileListInfo: FileListInfo,
	existingFiles: AudioFile[],
): AppEffect<void, never> {
	return Effect.gen(function* () {
		const staged = yield* stagePendingMetadataDrafts(services);
		if (!staged) {
			services.setFileImportError('Fix metadata validation errors before adding files.');
			return;
		}

		const appendOutcome = appendAnalyzedFiles(services, fileListInfo, existingFiles);
		if (appendOutcome !== 'duplicateOnly') {
			services.clearFileImportError();
		}
	});
}

function processAnalyzedFileList(
	evaluateFileListInfo: () => ReturnType<ImportAnalysisWorkflowServices['analyzeAudioFiles']>,
	existingFiles: AudioFile[],
): AppEffect<void, never, ImportAnalysisWorkflowServicesId> {
	return Effect.gen(function* () {
		const services = yield* ImportAnalysisWorkflowServicesTag;
		const fileListInfo = yield* analyzeFileListInfo(services, evaluateFileListInfo);
		if (!fileListInfo) {
			return;
		}

		yield* stageAndAppendAnalyzedFiles(services, fileListInfo, existingFiles);
	});
}

function processFilePaths(
	filePaths: string[],
	existingFiles: AudioFile[],
): AppEffect<void, never, ImportAnalysisWorkflowServicesId> {
	return Effect.gen(function* () {
		if (filePaths.length === 0) {
			return;
		}

		const services = yield* ImportAnalysisWorkflowServicesTag;
		yield* processAnalyzedFileList(() => services.analyzeAudioFiles(filePaths), existingFiles);
	});
}

function reportUnsupportedImport(
	services: ImportAnalysisWorkflowServices,
): AppEffect<void, never, ImportAnalysisWorkflowServicesId> {
	return Effect.gen(function* () {
		const metadata = yield* workflowPromise(
			() => services.getSupportedAudioImportMetadata(),
			'Failed to load supported audio import metadata.',
		).pipe(
			Effect.catchAll((error) =>
				Effect.sync(() => {
					return reportImportMetadataFailure(services, error.cause);
				}),
			),
		);

		if (metadata) {
			services.setFileImportError(unsupportedImportMessage(metadata));
		}
	});
}

function processDiscoveredPaths(
	discoveredPaths: string[],
	existingFiles: AudioFile[],
): AppEffect<void, never, ImportAnalysisWorkflowServicesId> {
	return Effect.gen(function* () {
		const services = yield* ImportAnalysisWorkflowServicesTag;

		if (discoveredPaths.length === 0) {
			yield* reportUnsupportedImport(services);
			return;
		}

		yield* processFilePaths(discoveredPaths, existingFiles);
	});
}

function discoverAndProcessPaths(
	paths: string[],
	existingFiles: AudioFile[],
): AppEffect<void, never, ImportAnalysisWorkflowServicesId> {
	return Effect.gen(function* () {
		const services = yield* ImportAnalysisWorkflowServicesTag;
		const discoveredPaths = yield* workflowPromise(
			() => services.discoverAudioImportPaths(paths),
			'Failed to discover audio files.',
		).pipe(
			Effect.catchAll((error) =>
				Effect.sync(() => {
					return reportImportDiscoveryFailure(services, error.cause);
				}),
			),
		);

		if (!discoveredPaths) {
			return;
		}

		yield* processDiscoveredPaths(discoveredPaths, existingFiles);
	});
}

function processPreparedFileList(
	fileListInfoPromise: ReturnType<ImportAnalysisWorkflowServices['analyzeAudioFiles']>,
	existingFiles: AudioFile[],
): AppEffect<void, never, ImportAnalysisWorkflowServicesId> {
	return processAnalyzedFileList(() => fileListInfoPromise, existingFiles);
}

function clickToSelect(
	existingFiles: AudioFile[],
): AppEffect<void, never, ImportAnalysisWorkflowServicesId> {
	return Effect.gen(function* () {
		const services = yield* ImportAnalysisWorkflowServicesTag;
		if (services.isOrderLocked()) {
			services.setFileImportError(importOrderLockedMessage());
			return;
		}

		yield* processSelectedFiles(services, () => openSupportedAudioFiles(services), existingFiles);
	});
}

function clickToSelectFolder(
	existingFiles: AudioFile[],
): AppEffect<void, never, ImportAnalysisWorkflowServicesId> {
	return Effect.gen(function* () {
		const services = yield* ImportAnalysisWorkflowServicesTag;
		if (services.isOrderLocked()) {
			services.setFileImportError(importOrderLockedMessage());
			return;
		}

		yield* processSelectedFolder(services, () => services.openDirectory(), existingFiles);
	});
}

function openSupportedAudioFiles(
	services: ImportAnalysisWorkflowServices,
): ReturnType<ImportAnalysisWorkflowServices['openFiles']> {
	return services.getSupportedAudioImportMetadata().then((metadata) =>
		services.openFiles({
			filters: [
				{
					name: 'Audio Files',
					extensions: [...metadata.extensions],
				},
			],
		}),
	);
}

function processSelectedFiles(
	services: ImportAnalysisWorkflowServices,
	evaluateSelectedPaths: () => ReturnType<ImportAnalysisWorkflowServices['openFiles']>,
	existingFiles: AudioFile[],
): AppEffect<void, never, ImportAnalysisWorkflowServicesId> {
	return Effect.gen(function* () {
		const selected = yield* workflowPromise(
			evaluateSelectedPaths,
			'Failed to open file dialog.',
		).pipe(
			Effect.catchAll((error) =>
				Effect.sync(() => {
					return reportOpenFileDialogFailure(services, error.cause);
				}),
			),
		);

		if (Array.isArray(selected) && selected.length > 0) {
			yield* discoverAndProcessPaths(selected, existingFiles);
		}
	});
}

function processSelectedFolder(
	services: ImportAnalysisWorkflowServices,
	evaluateSelectedPath: () => ReturnType<ImportAnalysisWorkflowServices['openDirectory']>,
	existingFiles: AudioFile[],
): AppEffect<void, never, ImportAnalysisWorkflowServicesId> {
	return Effect.gen(function* () {
		const selected = yield* workflowPromise(
			evaluateSelectedPath,
			'Failed to open folder dialog.',
		).pipe(
			Effect.catchAll((error) =>
				Effect.sync(() => {
					return reportOpenFolderDialogFailure(services, error.cause);
				}),
			),
		);

		if (selected) {
			yield* discoverAndProcessPaths([selected], existingFiles);
		}
	});
}

function clickToSelectFromPrepared(
	preparedEntry: Extract<PreparedImportAnalysisWorkflowEntry, { type: 'openFiles' }>,
): AppEffect<void, never, ImportAnalysisWorkflowServicesId> {
	return Effect.gen(function* () {
		const services = yield* ImportAnalysisWorkflowServicesTag;
		yield* processSelectedFiles(
			services,
			() => preparedEntry.selectedPaths,
			preparedEntry.existingFiles,
		);
	});
}

function clickToSelectFolderFromPrepared(
	preparedEntry: Extract<PreparedImportAnalysisWorkflowEntry, { type: 'openDirectory' }>,
): AppEffect<void, never, ImportAnalysisWorkflowServicesId> {
	return Effect.gen(function* () {
		const services = yield* ImportAnalysisWorkflowServicesTag;
		yield* processSelectedFolder(
			services,
			() => preparedEntry.selectedPath,
			preparedEntry.existingFiles,
		);
	});
}

function importPathsFromPrepared(
	preparedEntry: Extract<PreparedImportAnalysisWorkflowEntry, { type: 'discoverPaths' }>,
): AppEffect<void, never, ImportAnalysisWorkflowServicesId> {
	return Effect.gen(function* () {
		const services = yield* ImportAnalysisWorkflowServicesTag;
		const discoveredPaths = yield* workflowPromise(
			() => preparedEntry.discoveredPaths,
			'Failed to discover audio files.',
		).pipe(
			Effect.catchAll((error) =>
				Effect.sync(() => {
					return reportImportDiscoveryFailure(services, error.cause);
				}),
			),
		);

		if (discoveredPaths) {
			yield* processDiscoveredPaths(discoveredPaths, preparedEntry.existingFiles);
		}
	});
}

function importAnalysisWorkflowBody(
	action: ImportAnalysisWorkflowAction,
	preparedEntry?: PreparedImportAnalysisWorkflowEntry,
): AppEffect<void, never, ImportAnalysisWorkflowServicesId> {
	if (preparedEntry?.type === 'openFiles') {
		return clickToSelectFromPrepared(preparedEntry);
	}
	if (preparedEntry?.type === 'openDirectory') {
		return clickToSelectFolderFromPrepared(preparedEntry);
	}
	if (preparedEntry?.type === 'discoverPaths') {
		return importPathsFromPrepared(preparedEntry);
	}
	if (preparedEntry?.type === 'analyzeFiles') {
		return processPreparedFileList(preparedEntry.fileListInfo, preparedEntry.existingFiles);
	}

	switch (action.type) {
		case 'clickToSelect':
			return clickToSelect(action.existingFiles);
		case 'clickToSelectFolder':
			return clickToSelectFolder(action.existingFiles);
		case 'importPaths':
			return discoverAndProcessPaths(action.paths, action.existingFiles);
	}
}

export function enterImportAnalysisWorkflow(
	services: ImportAnalysisWorkflowServices,
	action: ImportAnalysisWorkflowAction,
): PreparedImportAnalysisWorkflowEntry | null {
	if (services.isOrderLocked()) {
		services.setFileImportError(importOrderLockedMessage());
		return null;
	}

	if (action.type === 'clickToSelect') {
		try {
			return {
				type: 'openFiles',
				selectedPaths: openSupportedAudioFiles(services),
				existingFiles: action.existingFiles,
			};
		} catch (cause) {
			reportOpenFileDialogFailure(services, cause);
			return null;
		}
	}

	if (action.type === 'clickToSelectFolder') {
		try {
			return {
				type: 'openDirectory',
				selectedPath: services.openDirectory(),
				existingFiles: action.existingFiles,
			};
		} catch (cause) {
			reportOpenFolderDialogFailure(services, cause);
			return null;
		}
	}

	try {
		return {
			type: 'discoverPaths',
			discoveredPaths: services.discoverAudioImportPaths(action.paths),
			existingFiles: action.existingFiles,
		};
	} catch (cause) {
		reportImportDiscoveryFailure(services, cause);
		return null;
	}
}

export function importAnalysisWorkflowExecution(
	action: ImportAnalysisWorkflowAction,
): AppEffect<void, never, ImportAnalysisWorkflowServicesId> {
	return importAnalysisWorkflowBody(action);
}

export async function runImportAnalysisWorkflow(
	action: ImportAnalysisWorkflowAction,
	layer?: ImportAnalysisWorkflowLayer,
	preparedEntry?: PreparedImportAnalysisWorkflowEntry,
): Promise<void> {
	const workflowLayer = layer ?? ImportAnalysisWorkflowLive;
	return runAppEffect(
		importAnalysisWorkflowBody(action, preparedEntry).pipe(Effect.provide(workflowLayer)),
	);
}
