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
	SUPPORTED_AUDIO_EXTENSIONS,
	SUPPORTED_AUDIO_FORMATS_TEXT,
	isSupportedAudioPath,
} from './supportedAudio';
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

function filterSupportedFiles(paths: string[]): string[] {
	return paths.filter((path) => isSupportedAudioPath(path));
}

function orderLockedMessage(): string {
	return 'Order locked while processing. Wait for completion to add files.';
}

function unsupportedDropMessage(): string {
	return `No supported audio files dropped. Please use ${SUPPORTED_AUDIO_FORMATS_TEXT} files.`;
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
			services.setFileImportError(orderLockedMessage());
			return;
		}

		const selected = yield* workflowPromise(
			() =>
				services.openFiles({
					filters: [
						{
							name: 'Audio Files',
							extensions: [...SUPPORTED_AUDIO_EXTENSIONS],
						},
					],
				}),
			'Failed to open file dialog.',
		).pipe(
			Effect.catchAll((error) =>
				Effect.sync(() => {
					return reportOpenFileDialogFailure(services, error.cause);
				}),
			),
		);

		if (Array.isArray(selected) && selected.length > 0) {
			yield* processFilePaths(selected, existingFiles);
		}
	});
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
			yield* processFilePaths(selected, preparedEntry.existingFiles);
		}
	});
}

function dropFiles(
	paths: string[],
	existingFiles: AudioFile[],
): AppEffect<void, never, ImportAnalysisWorkflowServicesId> {
	return Effect.gen(function* () {
		const services = yield* ImportAnalysisWorkflowServicesTag;
		if (services.isOrderLocked()) {
			services.setFileImportError(orderLockedMessage());
			return;
		}

		const supportedPaths = filterSupportedFiles(paths);
		if (supportedPaths.length === 0) {
			services.setFileImportError(unsupportedDropMessage());
			return;
		}

		yield* processFilePaths(supportedPaths, existingFiles);
	});
}

function importAnalysisWorkflowBody(
	action: ImportAnalysisWorkflowAction,
	preparedEntry?: PreparedImportAnalysisWorkflowEntry,
): AppEffect<void, never, ImportAnalysisWorkflowServicesId> {
	if (preparedEntry?.type === 'openFiles') {
		return clickToSelectFromPrepared(preparedEntry);
	}
	if (preparedEntry?.type === 'analyzeFiles') {
		return processPreparedFileList(preparedEntry.fileListInfo, preparedEntry.existingFiles);
	}

	switch (action.type) {
		case 'clickToSelect':
			return clickToSelect(action.existingFiles);
		case 'dropFiles':
			return dropFiles(action.paths, action.existingFiles);
	}
}

export function enterImportAnalysisWorkflow(
	services: ImportAnalysisWorkflowServices,
	action: ImportAnalysisWorkflowAction,
): PreparedImportAnalysisWorkflowEntry | null {
	if (services.isOrderLocked()) {
		services.setFileImportError(orderLockedMessage());
		return null;
	}

	if (action.type === 'clickToSelect') {
		try {
			return {
				type: 'openFiles',
				selectedPaths: services.openFiles({
					filters: [
						{
							name: 'Audio Files',
							extensions: [...SUPPORTED_AUDIO_EXTENSIONS],
						},
					],
				}),
				existingFiles: action.existingFiles,
			};
		} catch (cause) {
			reportOpenFileDialogFailure(services, cause);
			return null;
		}
	}

	const supportedPaths = filterSupportedFiles(action.paths);
	if (supportedPaths.length === 0) {
		services.setFileImportError(unsupportedDropMessage());
		return null;
	}

	try {
		return {
			type: 'analyzeFiles',
			fileListInfo: services.analyzeAudioFiles(supportedPaths),
			existingFiles: action.existingFiles,
		};
	} catch (cause) {
		reportAnalysisFailure(services, cause);
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
