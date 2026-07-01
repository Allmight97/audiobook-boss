import {
	Effect,
	type AppEffect,
	type AppLayer,
	makeWorkflowKit,
	runAppEffect,
} from '../../lib/effect/appEffect';
import { tauriClient } from '../../lib/tauri/client';
import type { AudioFile, FileListInfo } from '../../types/audio';
import {
	appendFileList,
	isOrderLocked,
	persistPendingMetadataDraftsForCurrentSelection,
} from '../fileList';
import { pushStatusPanelTransientStatus } from '../statusPanel';
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
import { clearFileImportError, setFileImportError } from './state.svelte';

export interface ImportAnalysisWorkflowServices {
	isOrderLocked: typeof isOrderLocked;
	getSupportedAudioImportMetadata: typeof tauriClient.getSupportedAudioImportMetadata;
	openFiles: typeof tauriClient.openFiles;
	openDirectory: typeof tauriClient.openDirectory;
	discoverAudioImportPaths: typeof tauriClient.discoverAudioImportPaths;
	analyzeAudioFiles: typeof tauriClient.analyzeAudioFiles;
	persistPendingMetadataDraftsForCurrentSelection: typeof persistPendingMetadataDraftsForCurrentSelection;
	appendFileList: typeof appendFileList;
	pushStatusPanelTransientStatus: typeof pushStatusPanelTransientStatus;
	setFileImportError: typeof setFileImportError;
	clearFileImportError: typeof clearFileImportError;
	console: Pick<Console, 'error'>;
}

export type ImportAnalysisWorkflowAction =
	| { type: 'clickToSelect'; existingFiles: AudioFile[] }
	| { type: 'clickToSelectFolder'; existingFiles: AudioFile[] }
	| { type: 'importPaths'; paths: string[]; existingFiles: AudioFile[] };

export type ImportAnalysisWorkflowServicesId = 'FileImport/ImportAnalysisWorkflowServices';
export type ImportAnalysisWorkflowLayer = AppLayer<ImportAnalysisWorkflowServicesId>;

const kit = makeWorkflowKit(
	'FileImport/ImportAnalysisWorkflowServices',
	'ImportAnalysisWorkflowFailed',
)<ImportAnalysisWorkflowServices>();

export const ImportAnalysisWorkflowServicesTag = kit.Tag;

export function makeImportAnalysisWorkflowServicesLayer(
	services: ImportAnalysisWorkflowServices,
): ImportAnalysisWorkflowLayer {
	return kit.makeLive(services);
}

export const liveImportAnalysisWorkflowServices = {
	isOrderLocked,
	getSupportedAudioImportMetadata: tauriClient.getSupportedAudioImportMetadata,
	openFiles: tauriClient.openFiles,
	openDirectory: tauriClient.openDirectory,
	discoverAudioImportPaths: tauriClient.discoverAudioImportPaths,
	analyzeAudioFiles: tauriClient.analyzeAudioFiles,
	persistPendingMetadataDraftsForCurrentSelection,
	appendFileList,
	pushStatusPanelTransientStatus,
	setFileImportError,
	clearFileImportError,
	console,
} satisfies ImportAnalysisWorkflowServices;

export const ImportAnalysisWorkflowLive = makeImportAnalysisWorkflowServicesLayer(
	liveImportAnalysisWorkflowServices,
);

export { importOrderLockedMessage } from './importAnalysisWorkflowFeedback';

export const ImportAnalysisWorkflowFailed = kit.Failed;
export type ImportAnalysisWorkflowFailed = InstanceType<typeof kit.Failed>;

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
	  };

export type ImportAnalysisWorkflowResult =
	| { status: 'imported' }
	| { status: 'blocked'; message: string };

function importedResult(): ImportAnalysisWorkflowResult {
	return { status: 'imported' };
}

function blockedResult(message: string): ImportAnalysisWorkflowResult {
	return { status: 'blocked', message };
}

const workflowPromise = kit.tryPromise;

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
): AppEffect<ImportAnalysisWorkflowResult, never> {
	return Effect.gen(function* () {
		const staged = yield* stagePendingMetadataDrafts(services);
		if (!staged) {
			const message = 'Fix metadata validation errors before adding files.';
			services.setFileImportError(message);
			return blockedResult(message);
		}

		const appendOutcome = appendAnalyzedFiles(services, fileListInfo, existingFiles);
		if (appendOutcome === 'duplicateOnly') {
			return blockedResult(duplicateOnlyImportMessage());
		}
		services.clearFileImportError();
		return importedResult();
	});
}

function processAnalyzedFileList(
	evaluateFileListInfo: () => ReturnType<ImportAnalysisWorkflowServices['analyzeAudioFiles']>,
	existingFiles: AudioFile[],
): AppEffect<ImportAnalysisWorkflowResult, never, ImportAnalysisWorkflowServicesId> {
	return Effect.gen(function* () {
		const services = yield* ImportAnalysisWorkflowServicesTag;
		const fileListInfo = yield* analyzeFileListInfo(services, evaluateFileListInfo);
		if (!fileListInfo) {
			return blockedResult('Failed to analyze files. Please try again.');
		}

		return yield* stageAndAppendAnalyzedFiles(services, fileListInfo, existingFiles);
	});
}

function processFilePaths(
	filePaths: string[],
	existingFiles: AudioFile[],
): AppEffect<ImportAnalysisWorkflowResult, never, ImportAnalysisWorkflowServicesId> {
	return Effect.gen(function* () {
		if (filePaths.length === 0) {
			return blockedResult('No audio files selected.');
		}

		const services = yield* ImportAnalysisWorkflowServicesTag;
		return yield* processAnalyzedFileList(
			() => services.analyzeAudioFiles(filePaths),
			existingFiles,
		);
	});
}

function reportUnsupportedImport(
	services: ImportAnalysisWorkflowServices,
): AppEffect<ImportAnalysisWorkflowResult, never, ImportAnalysisWorkflowServicesId> {
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
			const message = unsupportedImportMessage(metadata);
			services.setFileImportError(message);
			return blockedResult(message);
		}
		return blockedResult('Failed to load supported audio formats. Please try again.');
	});
}

function processDiscoveredPaths(
	discoveredPaths: string[],
	existingFiles: AudioFile[],
): AppEffect<ImportAnalysisWorkflowResult, never, ImportAnalysisWorkflowServicesId> {
	return Effect.gen(function* () {
		const services = yield* ImportAnalysisWorkflowServicesTag;

		if (discoveredPaths.length === 0) {
			return yield* reportUnsupportedImport(services);
		}

		return yield* processFilePaths(discoveredPaths, existingFiles);
	});
}

function discoverAndProcessPaths(
	paths: string[],
	existingFiles: AudioFile[],
): AppEffect<ImportAnalysisWorkflowResult, never, ImportAnalysisWorkflowServicesId> {
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
			return blockedResult('Failed to discover audio files. Please try again.');
		}

		return yield* processDiscoveredPaths(discoveredPaths, existingFiles);
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
): AppEffect<ImportAnalysisWorkflowResult, never, ImportAnalysisWorkflowServicesId> {
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
			return yield* discoverAndProcessPaths(selected, existingFiles);
		}
		return blockedResult('No audio files selected.');
	});
}

function processSelectedFolder(
	services: ImportAnalysisWorkflowServices,
	evaluateSelectedPath: () => ReturnType<ImportAnalysisWorkflowServices['openDirectory']>,
	existingFiles: AudioFile[],
): AppEffect<ImportAnalysisWorkflowResult, never, ImportAnalysisWorkflowServicesId> {
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
			return yield* discoverAndProcessPaths([selected], existingFiles);
		}
		return blockedResult('No audio files selected.');
	});
}

function clickToSelectFromPrepared(
	preparedEntry: Extract<PreparedImportAnalysisWorkflowEntry, { type: 'openFiles' }>,
): AppEffect<ImportAnalysisWorkflowResult, never, ImportAnalysisWorkflowServicesId> {
	return Effect.gen(function* () {
		const services = yield* ImportAnalysisWorkflowServicesTag;
		return yield* processSelectedFiles(
			services,
			() => preparedEntry.selectedPaths,
			preparedEntry.existingFiles,
		);
	});
}

function clickToSelectFolderFromPrepared(
	preparedEntry: Extract<PreparedImportAnalysisWorkflowEntry, { type: 'openDirectory' }>,
): AppEffect<ImportAnalysisWorkflowResult, never, ImportAnalysisWorkflowServicesId> {
	return Effect.gen(function* () {
		const services = yield* ImportAnalysisWorkflowServicesTag;
		return yield* processSelectedFolder(
			services,
			() => preparedEntry.selectedPath,
			preparedEntry.existingFiles,
		);
	});
}

function importPathsFromPrepared(
	preparedEntry: Extract<PreparedImportAnalysisWorkflowEntry, { type: 'discoverPaths' }>,
): AppEffect<ImportAnalysisWorkflowResult, never, ImportAnalysisWorkflowServicesId> {
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
			return yield* processDiscoveredPaths(discoveredPaths, preparedEntry.existingFiles);
		}
		return blockedResult('Failed to discover audio files. Please try again.');
	});
}

function importAnalysisWorkflowBody(
	preparedEntry: PreparedImportAnalysisWorkflowEntry,
): AppEffect<ImportAnalysisWorkflowResult, never, ImportAnalysisWorkflowServicesId> {
	switch (preparedEntry.type) {
		case 'openFiles':
			return clickToSelectFromPrepared(preparedEntry);
		case 'openDirectory':
			return clickToSelectFolderFromPrepared(preparedEntry);
		case 'discoverPaths':
			return importPathsFromPrepared(preparedEntry);
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

export async function runImportAnalysisWorkflow(
	preparedEntry: PreparedImportAnalysisWorkflowEntry,
	layer: ImportAnalysisWorkflowLayer = ImportAnalysisWorkflowLive,
): Promise<ImportAnalysisWorkflowResult> {
	return runAppEffect(importAnalysisWorkflowBody(preparedEntry).pipe(Effect.provide(layer)));
}
