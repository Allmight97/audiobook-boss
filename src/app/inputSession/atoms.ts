import type { JobType } from '../../types/audio';
import { Effect } from '../../lib/effect/appEffect';
import { toUserMessage } from '../../lib/tauri/appError';
import { liveInputCapability, type InputCapability } from '../../lib/tauri/capabilities/input';
import { formatDuration, formatFileSize, type AudioFile } from '../../types/audio';
import { Atom } from '../runtime/reactivity';
import { buildFileListAppendResult } from './appendResult';
import {
	clearAllFilesFromSession,
	moveFileInSession,
	removeFileFromSession,
	reorderFilesInSession,
	restoreImportOrderInSession,
	setOrderLockedInSession,
	sortFilesInSession,
} from './order';
import { clearSelectionInSession, selectAllInSession, selectFileInSession } from './selection';
import {
	DEFAULT_SUPPORT_TEXT,
	emptyInputSession,
	orderDiffersFromImport,
	type ImportIntent,
	type InputSessionState,
	type InputView,
	type SelectionModifiers,
} from './types';

export const inputCapabilityAtom = Atom.make<InputCapability>(liveInputCapability).pipe(
	Atom.keepAlive,
);

export const inputSessionAtom = Atom.make<InputSessionState>(emptyInputSession()).pipe(
	Atom.keepAlive,
);

export const jobTypeAtom = Atom.make<JobType>('batch').pipe(Atom.keepAlive);

export const inputViewAtom = Atom.make((get): InputView => toInputView(get(inputSessionAtom))).pipe(
	Atom.keepAlive,
);

export const importIntentAtom = Atom.fn((intent: ImportIntent, get) => {
	const capability = get(inputCapabilityAtom);
	const session = get(inputSessionAtom);
	return runImportIntent(capability, session, intent).pipe(
		Effect.tap((next) =>
			Effect.sync(() => {
				const current = get(inputSessionAtom);
				get.set(inputSessionAtom, { ...next, orderLocked: current.orderLocked });
			}),
		),
	);
}).pipe(Atom.keepAlive);

export const hydrateSupportTextAtom = Atom.fn((_: undefined, get) => {
	const capability = get(inputCapabilityAtom);
	const session = get(inputSessionAtom);
	return Effect.tryPromise({
		try: () => capability.getSupportedAudioImportMetadata(),
		catch: (cause) => cause,
	}).pipe(
		Effect.match({
			onFailure: () => session,
			onSuccess: (metadata) => ({
				...session,
				supportText: metadata.supportText || DEFAULT_SUPPORT_TEXT,
			}),
		}),
		Effect.tap((next) =>
			Effect.sync(() => {
				const current = get(inputSessionAtom);
				get.set(inputSessionAtom, { ...current, supportText: next.supportText });
			}),
		),
	);
}).pipe(Atom.keepAlive);

export const selectFileAtom = Atom.fnSync(
	(command: { readonly index: number; readonly modifiers: SelectionModifiers }, get) => {
		const next = selectFileInSession(get(inputSessionAtom), command.index, command.modifiers);
		get.set(inputSessionAtom, next);
		return next;
	},
	{ initialValue: emptyInputSession() },
).pipe(Atom.keepAlive);

export const selectAllAtom = Atom.fnSync(
	(_: undefined, get) => {
		const next = selectAllInSession(get(inputSessionAtom));
		get.set(inputSessionAtom, next);
		return next;
	},
	{ initialValue: emptyInputSession() },
).pipe(Atom.keepAlive);

export const clearSelectionAtom = Atom.fnSync(
	(_: undefined, get) => {
		const next = clearSelectionInSession(get(inputSessionAtom));
		get.set(inputSessionAtom, next);
		return next;
	},
	{ initialValue: emptyInputSession() },
).pipe(Atom.keepAlive);

export const setDragOverAtom = Atom.fnSync(
	(isDragOver: boolean, get) => {
		const session = get(inputSessionAtom);
		if (session.isDragOver === isDragOver) {
			return session;
		}
		const next = { ...session, isDragOver };
		get.set(inputSessionAtom, next);
		return next;
	},
	{ initialValue: emptyInputSession() },
).pipe(Atom.keepAlive);

export const removeFileAtom = Atom.fnSync(
	(index: number, get) => {
		const { session } = removeFileFromSession(get(inputSessionAtom), index);
		get.set(inputSessionAtom, session);
		return session;
	},
	{ initialValue: emptyInputSession() },
).pipe(Atom.keepAlive);

export const clearAllFilesAtom = Atom.fnSync(
	(_: undefined, get) => {
		const next = clearAllFilesFromSession(get(inputSessionAtom));
		get.set(inputSessionAtom, next);
		return next;
	},
	{ initialValue: emptyInputSession() },
).pipe(Atom.keepAlive);

export const moveFileAtom = Atom.fnSync(
	(command: { readonly index: number; readonly direction: 'up' | 'down' }, get) => {
		const next = moveFileInSession(get(inputSessionAtom), command.index, command.direction);
		get.set(inputSessionAtom, next);
		return next;
	},
	{ initialValue: emptyInputSession() },
).pipe(Atom.keepAlive);

export const reorderFilesAtom = Atom.fnSync(
	(command: { readonly fromIndex: number; readonly toIndex: number }, get) => {
		const next = reorderFilesInSession(get(inputSessionAtom), command.fromIndex, command.toIndex);
		get.set(inputSessionAtom, next);
		return next;
	},
	{ initialValue: emptyInputSession() },
).pipe(Atom.keepAlive);

export const toggleSortAtom = Atom.fnSync(
	(_: undefined, get) => {
		const next = sortFilesInSession(get(inputSessionAtom));
		get.set(inputSessionAtom, next);
		return next;
	},
	{ initialValue: emptyInputSession() },
).pipe(Atom.keepAlive);

export const restoreImportOrderAtom = Atom.fnSync(
	(_: undefined, get) => {
		const next = restoreImportOrderInSession(get(inputSessionAtom));
		get.set(inputSessionAtom, next);
		return next;
	},
	{ initialValue: emptyInputSession() },
).pipe(Atom.keepAlive);

export const setOrderLockedAtom = Atom.fnSync(
	(orderLocked: boolean, get) => {
		const next = setOrderLockedInSession(get(inputSessionAtom), orderLocked);
		get.set(inputSessionAtom, next);
		return next;
	},
	{ initialValue: emptyInputSession() },
).pipe(Atom.keepAlive);

export const setJobTypeAtom = Atom.fnSync(
	(jobType: JobType, get) => {
		get.set(jobTypeAtom, jobType);
		return jobType;
	},
	{ initialValue: 'batch' as JobType },
).pipe(Atom.keepAlive);

export function displayedTitleForFile(file: AudioFile): string {
	if (file.tagTitle?.trim()) {
		return file.tagTitle;
	}
	const segments = file.path.split(/[\\/]/).filter((segment) => segment !== '');
	return segments[segments.length - 1] ?? file.path;
}

export function displayedArtistForFile(file: AudioFile): string {
	return file.tagArtist?.trim() ?? '';
}

export function formatFileDetails(file: AudioFile): string {
	const artist = displayedArtistForFile(file);
	const artistPrefix = artist ? `${artist} • ` : '';
	const chapterSuffix = file.chapters?.length
		? ` • ${file.chapters.length} chapter${file.chapters.length === 1 ? '' : 's'}`
		: '';
	if (file.isValid && file.duration && file.size) {
		return `${artistPrefix}${formatDuration(file.duration)} • ${formatFileSize(file.size)} • ${file.format}${chapterSuffix}`;
	}
	return `Error: ${file.error || 'Invalid file'}`;
}

function toInputView(session: InputSessionState): InputView {
	const files = session.fileList?.files ?? [];
	const locked = session.orderLocked;
	const differs = orderDiffersFromImport(files, session.importOrdinalByPath);
	return {
		files,
		selectedIndices: session.selectedIndices,
		selectedAnchor: session.selectedAnchor,
		fileCount: files.length,
		hasFiles: files.length > 0,
		orderLocked: locked,
		errorMessage: session.errorMessage,
		isDragOver: session.isDragOver,
		supportText: session.supportText,
		sortDirection: session.sortDirection,
		sortLabel: session.sortDirection === 'descending' ? 'Sort: Z-A' : 'Sort: A-Z',
		orderDiffersFromImport: differs,
		showSortButton: files.length > 1,
		showClearButton: files.length > 0,
		showRestoreImportOrder: differs && !locked,
		totalDurationSeconds: session.fileList?.totalDuration ?? 0,
	};
}

function runImportIntent(
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
