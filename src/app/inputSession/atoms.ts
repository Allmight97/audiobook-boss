import { Effect } from '../../lib/effect/appEffect';
import { toUserMessage } from '../../lib/tauri/appError';
import { liveInputCapability, type InputCapability } from '../../lib/tauri/capabilities/input';
import { formatDuration, formatFileSize, type AudioFile } from '../../types/audio';
import { Atom } from '../runtime/reactivity';
import { buildFileListAppendResult } from './appendResult';
import {
	DEFAULT_SUPPORT_TEXT,
	emptyInputSession,
	fileIdentityKey,
	type ImportIntent,
	type InputSessionState,
	type InputView,
	type InputViewFile,
	type SelectionModifiers,
} from './types';

export const inputCapabilityAtom = Atom.make<InputCapability>(liveInputCapability).pipe(
	Atom.keepAlive,
);

export const inputSessionAtom = Atom.make<InputSessionState>(emptyInputSession()).pipe(
	Atom.keepAlive,
);

export const inputViewAtom = Atom.make((get): InputView => toInputView(get(inputSessionAtom))).pipe(
	Atom.keepAlive,
);

export const importIntentAtom = Atom.fn((intent: ImportIntent, get) => {
	const capability = get(inputCapabilityAtom);
	const session = get(inputSessionAtom);
	return runImportIntent(capability, session, intent).pipe(
		Effect.tap((next) => Effect.sync(() => get.set(inputSessionAtom, next))),
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
		Effect.tap((next) => Effect.sync(() => get.set(inputSessionAtom, next))),
	);
}).pipe(Atom.keepAlive);

export const selectFileAtom = Atom.fnSync(
	(command: { readonly index: number; readonly modifiers: SelectionModifiers }, get) => {
		const session = get(inputSessionAtom);
		const next = selectFileInSession(session, command.index, command.modifiers);
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
	const selected = new Set(session.selectedIndices);
	const viewFiles: InputViewFile[] = files.map((file, index) => ({
		...file,
		index,
		selected: selected.has(index),
	}));
	return {
		files: viewFiles,
		selectedIndices: session.selectedIndices,
		fileCount: files.length,
		hasFiles: files.length > 0,
		orderLocked: session.orderLocked,
		errorMessage: session.errorMessage,
		isDragOver: session.isDragOver,
		supportText: session.supportText,
		sortDirection: session.sortDirection,
		sortLabel:
			session.sortDirection === 'ascending'
				? 'A → Z'
				: session.sortDirection === 'descending'
					? 'Z → A'
					: 'Sort',
		orderDiffersFromImport: files.some(
			(file, index) => session.importOrdinalByPath[file.path] !== index,
		),
	};
}

function selectFileInSession(
	session: InputSessionState,
	index: number,
	modifiers: SelectionModifiers,
): InputSessionState {
	const files = session.fileList?.files ?? [];
	if (index < 0 || index >= files.length) {
		return session;
	}

	if (modifiers.range && session.selectedAnchor !== -1) {
		const start = Math.min(session.selectedAnchor, index);
		const end = Math.max(session.selectedAnchor, index);
		const selectedIndices = Array.from({ length: end - start + 1 }, (_, offset) => start + offset);
		return { ...session, selectedIndices, selectedAnchor: index };
	}

	if (modifiers.multi) {
		const selected = new Set(session.selectedIndices);
		if (selected.has(index)) {
			selected.delete(index);
		} else {
			selected.add(index);
		}
		const selectedIndices = Array.from(selected).sort((left, right) => left - right);
		const selectedAnchor = selected.has(index)
			? index
			: (selectedIndices[selectedIndices.length - 1] ?? -1);
		return { ...session, selectedIndices, selectedAnchor };
	}

	return { ...session, selectedIndices: [index], selectedAnchor: index };
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
		for (const file of appendResult.appendedFiles) {
			if (importOrdinalByPath[file.path] === undefined) {
				importOrdinalByPath[file.path] = nextImportOrdinal;
				nextImportOrdinal += 1;
			}
		}

		const appendedFile = appendResult.appendedFiles[0];
		const selectedIndex =
			appendedFile === undefined
				? -1
				: fileList.files.findIndex(
						(file) => fileIdentityKey(file) === fileIdentityKey(appendedFile),
					);
		return {
			...session,
			fileList,
			selectedIndices: selectedIndex >= 0 ? [selectedIndex] : [],
			selectedAnchor: selectedIndex,
			errorMessage: '',
			isDragOver: false,
			importOrdinalByPath,
			nextImportOrdinal,
		};
	});
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
