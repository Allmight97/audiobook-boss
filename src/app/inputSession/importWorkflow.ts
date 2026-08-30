import type { AudioFile } from '../../types/audio';
import { Effect } from '../../lib/effect/appEffect';
import { toUserMessage } from '../../lib/tauri/appError';
import type { InputCapability } from '../../lib/tauri/capabilities/input';
import { buildFileListAppendResult } from './appendResult';
import type { ImportIntent, InputSessionState } from './types';

export function runImportIntent(
	capability: InputCapability,
	session: InputSessionState,
	intent: ImportIntent,
): Effect.Effect<InputSessionState> {
	if (session.orderLocked) {
		return Effect.succeed({
			...session,
			errorMessage: 'Order locked while processing. Wait for completion to add files.',
		});
	}

	return Effect.gen(function* () {
		if (intent.type === 'pickFiles') {
			const selected = yield* tryUserAction(
				() => openSupportedAudioFiles(capability),
				'Failed to open file dialog. Please try again.',
			);
			if (!selected.ok) {
				return withError(session, selected.message);
			}
			if (!selected.value || selected.value.length === 0) {
				return session;
			}
			return yield* importDiscoveredPaths(capability, session, selected.value);
		}

		if (intent.type === 'pickFolder') {
			const selected = yield* tryUserAction(
				() => capability.openDirectory(),
				'Failed to open folder dialog. Please try again.',
			);
			if (!selected.ok) {
				return withError(session, selected.message);
			}
			if (!selected.value) {
				return session;
			}
			return yield* importDiscoveredPaths(capability, session, [selected.value]);
		}

		if (intent.type === 'drainOpened') {
			const opened = yield* tryUserAction(
				() => capability.takeOpenedAudioFiles(),
				'Failed to import opened audio files. Please try again.',
			);
			if (!opened.ok) {
				return withError(session, opened.message);
			}
			if (opened.value.length === 0) {
				return session;
			}
			return yield* importDiscoveredPaths(capability, session, opened.value);
		}

		return yield* importDiscoveredPaths(capability, session, [...intent.paths]);
	});
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

function importDiscoveredPaths(
	capability: InputCapability,
	session: InputSessionState,
	paths: string[],
): Effect.Effect<InputSessionState> {
	return Effect.gen(function* () {
		const discovered = yield* tryUserAction(
			() => capability.discoverAudioImportPaths(paths),
			'Failed to discover audio files. Please try again.',
		);
		if (!discovered.ok) {
			return withError(session, discovered.message);
		}
		if (discovered.value.length === 0) {
			const metadata = yield* tryUserAction(
				() => capability.getSupportedAudioImportMetadata(),
				'Failed to load supported audio formats. Please try again.',
			);
			if (!metadata.ok) {
				return withError(session, metadata.message);
			}
			return withError(
				session,
				`No supported audio files found. Please use ${metadata.value.formatsText} files.`,
			);
		}

		const analyzed = yield* tryUserAction(
			() => capability.analyzeAudioFiles(discovered.value),
			'Failed to analyze files. Please try again.',
		);
		if (!analyzed.ok) {
			return withError(session, analyzed.message);
		}

		const existingFiles = session.fileList?.files ?? [];
		const appendResult = buildFileListAppendResult(analyzed.value, {
			existingFiles,
			currentFileList: session.fileList,
		});
		if (appendResult.outcome === 'duplicateOnly') {
			return withError(session, 'No new files added. All analyzed files were already in the list.');
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
	});
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

function withError(session: InputSessionState, errorMessage: string): InputSessionState {
	return { ...session, errorMessage, isDragOver: false };
}

type UserActionResult<A> = { ok: true; value: A } | { ok: false; message: string };

function tryUserAction<A>(
	evaluate: () => Promise<A>,
	fallback: string,
): Effect.Effect<UserActionResult<A>> {
	return Effect.tryPromise({
		try: evaluate,
		catch: (cause) => cause,
	}).pipe(
		Effect.match({
			onFailure: (cause): UserActionResult<A> => ({
				ok: false,
				message: toUserMessage(cause, { fallback, suppressUnknown: true }),
			}),
			onSuccess: (value): UserActionResult<A> => ({ ok: true, value }),
		}),
	);
}
