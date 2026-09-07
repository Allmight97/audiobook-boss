import type { AudioFile, FileListInfo } from '../../types/audio';
import { toUserMessage } from '../../lib/tauri/appError';
import type { InputCapability } from '../../lib/tauri/capabilities/input';
import { buildFileListAppendResult } from './appendResult';
import type { ImportIntent, InputSessionState } from './types';

type ImportUpdate = (session: InputSessionState) => InputSessionState;

export async function runImportIntent(
	capability: InputCapability,
	session: InputSessionState,
	intent: ImportIntent,
): Promise<ImportUpdate> {
	if (session.orderLocked) {
		return withError('Order locked while processing. Wait for completion to add files.');
	}

	if (intent.type === 'pickFiles') {
		const selected = await tryUserAction(
			() => openSupportedAudioFiles(capability),
			'Failed to open file dialog. Please try again.',
		);
		if (!selected.ok) {
			return withError(selected.message);
		}
		if (!selected.value || selected.value.length === 0) {
			return (current) => current;
		}
		return importDiscoveredPaths(capability, selected.value);
	}

	if (intent.type === 'pickFolder') {
		const selected = await tryUserAction(
			() => capability.openDirectory(),
			'Failed to open folder dialog. Please try again.',
		);
		if (!selected.ok) {
			return withError(selected.message);
		}
		if (!selected.value) {
			return (current) => current;
		}
		return importDiscoveredPaths(capability, [selected.value]);
	}

	if (intent.type === 'drainOpened') {
		const opened = await tryUserAction(
			() => capability.takeOpenedAudioFiles(),
			'Failed to import opened audio files. Please try again.',
		);
		if (!opened.ok) {
			return withError(opened.message);
		}
		if (opened.value.length === 0) {
			return (current) => current;
		}
		return importDiscoveredPaths(capability, opened.value);
	}

	return importDiscoveredPaths(capability, [...intent.paths]);
}

async function openSupportedAudioFiles(capability: InputCapability): Promise<string[] | null> {
	const metadata = await capability.getSupportedAudioImportMetadata();
	return capability.openFiles({
		filters: [
			{
				name: 'Audio Files',
				extensions: [...metadata.extensions],
			},
		],
	});
}

async function importDiscoveredPaths(
	capability: InputCapability,
	paths: string[],
): Promise<ImportUpdate> {
	const discovered = await tryUserAction(
		() => capability.discoverAudioImportPaths(paths),
		'Failed to discover audio files. Please try again.',
	);
	if (!discovered.ok) {
		return withError(discovered.message);
	}
	if (discovered.value.length === 0) {
		const metadata = await tryUserAction(
			() => capability.getSupportedAudioImportMetadata(),
			'Failed to load supported audio formats. Please try again.',
		);
		if (!metadata.ok) {
			return withError(metadata.message);
		}
		return withError(
			`No supported audio files found. Please use ${metadata.value.formatsText} files.`,
		);
	}

	const analyzed = await tryUserAction(
		() => capability.analyzeAudioFiles(discovered.value),
		'Failed to analyze files. Please try again.',
	);
	if (!analyzed.ok) {
		return withError(analyzed.message);
	}

	return (session) => appendAnalyzedFiles(session, analyzed.value);
}

function appendAnalyzedFiles(
	session: InputSessionState,
	analyzed: FileListInfo,
): InputSessionState {
	const existingFiles = session.fileList?.files ?? [];
	const appendResult = buildFileListAppendResult(analyzed, {
		existingFiles,
		currentFileList: session.fileList,
	});
	if (appendResult.outcome === 'duplicateOnly') {
		return withError('No new files added. All analyzed files were already in the list.')(session);
	}

	const fileList = appendResult.fileList;
	const importOrdinalByPath = { ...session.importOrdinalByPath };
	let nextImportOrdinal = session.nextImportOrdinal;
	if (appendResult.outcome === 'replace') {
		for (const key of Object.keys(importOrdinalByPath)) {
			delete importOrdinalByPath[key];
		}
		nextImportOrdinal = 0;
	}
	for (const file of appendResult.appendedFiles) {
		if (importOrdinalByPath[file.path] === undefined) {
			importOrdinalByPath[file.path] = nextImportOrdinal;
			nextImportOrdinal += 1;
		}
	}

	const selected = selectionAfterAppend(session, appendResult.outcome, fileList.files);
	return {
		...session,
		fileList,
		selectedIndices: selected.selectedIndices,
		selectedAnchor: selected.selectedAnchor,
		errorMessage: '',
		isDragOver: false,
		importOrdinalByPath,
		nextImportOrdinal,
		sortDirection: appendResult.outcome === 'replace' ? 'none' : session.sortDirection,
	};
}

function selectionAfterAppend(
	session: InputSessionState,
	outcome: 'replace' | 'append',
	files: ReadonlyArray<AudioFile>,
): { selectedIndices: ReadonlyArray<number>; selectedAnchor: number } {
	if (outcome === 'replace') {
		if (files.length === 1 && files[0]?.isValid) {
			return { selectedIndices: [0], selectedAnchor: 0 };
		}
		return { selectedIndices: [], selectedAnchor: -1 };
	}
	return {
		selectedIndices: session.selectedIndices,
		selectedAnchor: session.selectedAnchor,
	};
}

function withError(errorMessage: string): ImportUpdate {
	return (session) => ({ ...session, errorMessage, isDragOver: false });
}

type UserActionResult<A> = { ok: true; value: A } | { ok: false; message: string };

async function tryUserAction<A>(
	evaluate: () => Promise<A>,
	fallback: string,
): Promise<UserActionResult<A>> {
	try {
		return { ok: true, value: await evaluate() };
	} catch (cause) {
		return {
			ok: false,
			message: toUserMessage(cause, { fallback, suppressUnknown: true }),
		};
	}
}
