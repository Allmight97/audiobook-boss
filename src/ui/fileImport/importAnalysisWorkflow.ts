import {
	Data,
	Effect,
	type AppEffect,
	runAppEffect,
	workflowTryPromise,
} from '../../lib/effect/appEffect';
import type { AudioFile, FileListInfo, SupportedAudioImportMetadata } from '../../types/audio';
import { ImportAnalysisWorkflowLive } from './importAnalysisWorkflowLive';
import {
	ImportAnalysisWorkflowServicesTag,
	type ImportAnalysisWorkflowAction,
	type ImportAnalysisWorkflowLayer,
	type ImportAnalysisWorkflowServices,
	type ImportAnalysisWorkflowServicesId,
} from './importAnalysisWorkflowServices';

export {
	ImportAnalysisWorkflowServicesTag,
	makeImportAnalysisWorkflowServicesLayer,
	type ImportAnalysisFileListResult,
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

export function importOrderLockedMessage(): string {
	return 'Order locked while processing. Wait for completion to add files.';
}

function unsupportedImportMessage(metadata: SupportedAudioImportMetadata): string {
	return `No supported audio files found. Please use ${metadata.formatsText} files.`;
}

function reportAnalysisFailure(
	services: ImportAnalysisWorkflowServices,
	cause: unknown,
): FileListInfo | null {
	services.console.error('Failed to analyze files:', cause);
	services.setFileImportError('Failed to analyze files. Please try again.');
	return null;
}

function reportMetadataStagingFailure(
	services: ImportAnalysisWorkflowServices,
	cause: unknown,
): false {
	services.console.error('Failed to stage metadata drafts:', cause);
	services.setFileImportError(
		'Failed to prepare metadata drafts before adding files. Please try again.',
	);
	return false;
}

function reportOpenFileDialogFailure(
	services: ImportAnalysisWorkflowServices,
	cause: unknown,
): null {
	services.console.error('Failed to open file dialog:', cause);
	services.setFileImportError('Failed to open file dialog. Please try again.');
	return null;
}

function reportOpenFolderDialogFailure(
	services: ImportAnalysisWorkflowServices,
	cause: unknown,
): null {
	services.console.error('Failed to open folder dialog:', cause);
	services.setFileImportError('Failed to open folder dialog. Please try again.');
	return null;
}

function reportImportDiscoveryFailure(
	services: ImportAnalysisWorkflowServices,
	cause: unknown,
): string[] | null {
	services.console.error('Failed to discover audio files:', cause);
	services.setFileImportError('Failed to discover audio files. Please try again.');
	return null;
}

function reportImportMetadataFailure(
	services: ImportAnalysisWorkflowServices,
	cause: unknown,
): SupportedAudioImportMetadata | null {
	services.console.error('Failed to load supported audio import metadata:', cause);
	services.setFileImportError('Failed to load supported audio formats. Please try again.');
	return null;
}

function hasOnlyDuplicateFiles(fileListInfo: FileListInfo, existingFiles: AudioFile[]): boolean {
	if (existingFiles.length === 0 || fileListInfo.files.length === 0) {
		return false;
	}
	const existingPaths = new Set(existingFiles.map((file) => file.path));
	return fileListInfo.files.every((file) => existingPaths.has(file.path));
}

function appendAnalyzedFiles(
	services: ImportAnalysisWorkflowServices,
	fileListInfo: FileListInfo,
	existingFiles: AudioFile[],
): void {
	if (hasOnlyDuplicateFiles(fileListInfo, existingFiles)) {
		services.appendFileList(fileListInfo, {
			existingFiles,
			showDuplicateStatus: false,
		});
		services.pushStatusPanelTransientStatus(
			'No new files added. All analyzed files were already in the list.',
			{ ttlMs: 2000 },
		);
		return;
	}

	services.appendFileList(fileListInfo, { existingFiles });
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
		const fileListInfo = yield* workflowPromise(
			() => services.analyzeAudioFiles(filePaths),
			'Failed to analyze files.',
		).pipe(
			Effect.catchAll((error) =>
				Effect.sync(() => {
					return reportAnalysisFailure(services, error.cause);
				}),
			),
		);
		if (!fileListInfo) {
			return;
		}

		const staged = yield* workflowPromise(
			() => services.persistPendingMetadataDraftsForCurrentSelection(),
			'Failed to stage metadata drafts before import.',
		).pipe(
			Effect.catchAll((error) =>
				Effect.sync(() => {
					return reportMetadataStagingFailure(services, error.cause);
				}),
			),
		);
		if (!staged) {
			services.setFileImportError('Fix metadata validation errors before adding files.');
			return;
		}

		appendAnalyzedFiles(services, fileListInfo, existingFiles);
		services.clearFileImportError();
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
	return Effect.gen(function* () {
		const services = yield* ImportAnalysisWorkflowServicesTag;
		const fileListInfo = yield* workflowPromise(
			() => fileListInfoPromise,
			'Failed to analyze files.',
		).pipe(
			Effect.catchAll((error) =>
				Effect.sync(() => {
					return reportAnalysisFailure(services, error.cause);
				}),
			),
		);
		if (!fileListInfo) {
			return;
		}

		const staged = yield* workflowPromise(
			() => services.persistPendingMetadataDraftsForCurrentSelection(),
			'Failed to stage metadata drafts before import.',
		).pipe(
			Effect.catchAll((error) =>
				Effect.sync(() => {
					return reportMetadataStagingFailure(services, error.cause);
				}),
			),
		);
		if (!staged) {
			services.setFileImportError('Fix metadata validation errors before adding files.');
			return;
		}

		appendAnalyzedFiles(services, fileListInfo, existingFiles);
		services.clearFileImportError();
	});
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

		const selected = yield* workflowPromise(
			() => openSupportedAudioFiles(services),
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

function clickToSelectFolder(
	existingFiles: AudioFile[],
): AppEffect<void, never, ImportAnalysisWorkflowServicesId> {
	return Effect.gen(function* () {
		const services = yield* ImportAnalysisWorkflowServicesTag;
		if (services.isOrderLocked()) {
			services.setFileImportError(importOrderLockedMessage());
			return;
		}

		const selected = yield* workflowPromise(
			() => services.openDirectory(),
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

function clickToSelectFromPrepared(
	preparedEntry: Extract<PreparedImportAnalysisWorkflowEntry, { type: 'openFiles' }>,
): AppEffect<void, never, ImportAnalysisWorkflowServicesId> {
	return Effect.gen(function* () {
		const services = yield* ImportAnalysisWorkflowServicesTag;
		const selected = yield* workflowPromise(
			() => preparedEntry.selectedPaths,
			'Failed to open file dialog.',
		).pipe(
			Effect.catchAll((error) =>
				Effect.sync(() => {
					return reportOpenFileDialogFailure(services, error.cause);
				}),
			),
		);

		if (Array.isArray(selected) && selected.length > 0) {
			yield* discoverAndProcessPaths(selected, preparedEntry.existingFiles);
		}
	});
}

function clickToSelectFolderFromPrepared(
	preparedEntry: Extract<PreparedImportAnalysisWorkflowEntry, { type: 'openDirectory' }>,
): AppEffect<void, never, ImportAnalysisWorkflowServicesId> {
	return Effect.gen(function* () {
		const services = yield* ImportAnalysisWorkflowServicesTag;
		const selected = yield* workflowPromise(
			() => preparedEntry.selectedPath,
			'Failed to open folder dialog.',
		).pipe(
			Effect.catchAll((error) =>
				Effect.sync(() => {
					return reportOpenFolderDialogFailure(services, error.cause);
				}),
			),
		);

		if (selected) {
			yield* discoverAndProcessPaths([selected], preparedEntry.existingFiles);
		}
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
