import type { AudioFile, FileListInfo, JobType } from '../../types/audio';
import type { AudiobookMetadata } from '../../types/metadata';
import { coverArtBytesToDataUrl } from '../../lib/media/coverArtDataUrl';
import { toUserMessage } from '../../lib/tauri/appError';
import { Effect } from '../../lib/effect/appEffect';
import {
	liveMetadataCapability,
	type MetadataCapability,
} from '../../lib/tauri/capabilities/metadata';
import { Atom } from '../runtime/reactivity';
import { inputSessionAtom, jobTypeAtom } from '../inputSession/atoms';
import type { InputSessionState } from '../inputSession/types';
import {
	cacheMetadataForFile,
	clearMetadataSession,
	clearPendingMetadataForFile,
	getMetadataForFile,
	getMetadataIntentPatchForFile,
	getPendingMetadataIntentEntries,
	isUsableMetadataCache,
	removeMetadataForFile,
	stageMetadataIntentPatch,
} from './cache';
import {
	COVER_ART_IMAGE_EXTENSION_HINTS,
	COVER_ART_IMAGE_EXTENSION_HINT_PATTERN,
	createEmptyCoverUiState,
	formatCoverArtError,
	parseCoverArtUrl,
	type CoverUiState,
} from './cover';
import {
	effectiveCoverForFile,
	resolveCoverDisplayPath,
	resolveCoverOwnerPaths,
} from './coverOwner';
import {
	getMetadataFieldDefinitionByActionId,
	getMetadataFieldDefinitionByInputId,
	createEmptyFormState,
	replaceField,
	type MetadataFieldId,
	type MetadataFormState,
} from './fields';
import {
	applyFieldAction,
	applyFieldInput,
	applyMetadataFormValidationWarnings,
	applyMetadataToForm,
	commitFocusedControlValue,
	hasDirtyMetadataFields,
	populateMetadataFormMulti,
	populateMetadataFormSingle,
	readMetadataForm,
	readMetadataFormPreviewValues,
	resetDirtyState,
} from './form';
import {
	commitPreparedMetadataDrafts,
	prepareMetadataDrafts,
	readUncachedMetadataSnapshot,
} from './staging';
import { projectTagPreviewValues } from './tags';
import { buildMetadataDraftIntent } from './draft';

export type MetadataEditorState = {
	readonly form: MetadataFormState;
	readonly cover: CoverUiState;
	readonly saveInProgress: boolean;
	readonly formRevision: number;
	readonly coverRevision: number;
	readonly focusedFieldId: MetadataFieldId | null;
	readonly selectionKey: string;
	readonly boundFiles: ReadonlyArray<AudioFile>;
	readonly hydrateRequestId: number;
	readonly autoCoverRequestId: number;
	readonly statusMessage: string;
};

export type MetadataView = {
	readonly form: MetadataFormState;
	readonly cover: CoverUiState;
	readonly tags: ReturnType<typeof projectTagPreviewValues>;
	readonly saveInProgress: boolean;
	readonly focusedFieldId: MetadataFieldId | null;
	readonly statusMessage: string;
};

function emptyEditor(): MetadataEditorState {
	return {
		form: createEmptyFormState(),
		cover: createEmptyCoverUiState(),
		saveInProgress: false,
		formRevision: 0,
		coverRevision: 0,
		focusedFieldId: null,
		selectionKey: '',
		boundFiles: [],
		hydrateRequestId: 0,
		autoCoverRequestId: 0,
		statusMessage: '',
	};
}

function bumpForm(editor: MetadataEditorState, form: MetadataFormState): MetadataEditorState {
	return { ...editor, form, formRevision: editor.formRevision + 1 };
}

function bumpCover(editor: MetadataEditorState, cover: Partial<CoverUiState>): MetadataEditorState {
	return {
		...editor,
		cover: { ...editor.cover, ...cover },
		coverRevision: editor.coverRevision + 1,
	};
}

function selectionKeyFor(files: ReadonlyArray<AudioFile>): string {
	return files
		.map((file) => file.path)
		.sort()
		.join('\0');
}

function selectedFilesFromSession(session: {
	readonly fileList: FileListInfo | null;
	readonly selectedIndices: ReadonlyArray<number>;
}): AudioFile[] {
	const files = session.fileList?.files ?? [];
	return session.selectedIndices
		.map((index) => files[index])
		.filter((file): file is AudioFile => Boolean(file));
}

function displayCover(cover: CoverUiState, bytes: number[] | null): CoverUiState {
	return {
		...cover,
		currentCoverArt: bytes,
		imageDataUrl: bytes && bytes.length > 0 ? coverArtBytesToDataUrl(bytes) : null,
	};
}

let coverMessageTimeoutId: number | null = null;

function scheduleCoverMessageClear(getSet: (cover: Partial<CoverUiState>) => void): void {
	if (coverMessageTimeoutId !== null) {
		window.clearTimeout(coverMessageTimeoutId);
	}
	coverMessageTimeoutId = window.setTimeout(() => {
		coverMessageTimeoutId = null;
		getSet({ message: { kind: 'hidden' } });
	}, 4000);
}

export const metadataCapabilityAtom = Atom.make<MetadataCapability>(liveMetadataCapability).pipe(
	Atom.keepAlive,
);

export const metadataEditorAtom = Atom.make<MetadataEditorState>(emptyEditor()).pipe(
	Atom.keepAlive,
);

export const metadataViewAtom = Atom.make((get): MetadataView => {
	const editor = get(metadataEditorAtom);
	return {
		form: editor.form,
		cover: editor.cover,
		tags: projectTagPreviewValues(readMetadataFormPreviewValues(editor.form)),
		saveInProgress: editor.saveInProgress,
		focusedFieldId: editor.focusedFieldId,
		statusMessage: editor.statusMessage,
	};
}).pipe(Atom.keepAlive);

function syncRemovedFiles(sessionFiles: ReadonlyArray<AudioFile>): void {
	const livePaths = new Set(sessionFiles.map((file) => file.path));
	for (const [filePath] of getPendingMetadataIntentEntries()) {
		if (!livePaths.has(filePath)) {
			removeMetadataForFile(filePath);
		}
	}
}

async function loadMetadataForFile(
	capability: MetadataCapability,
	file: AudioFile,
): Promise<Partial<AudiobookMetadata> | null> {
	if (!file.isValid) return null;
	const existing = getMetadataForFile(file.path);
	if (isUsableMetadataCache(existing)) return existing;
	try {
		const metadata = await capability.readAudioMetadata(file.path);
		cacheMetadataForFile(file.path, metadata);
		return metadata;
	} catch (error) {
		console.warn('Failed to load metadata:', error);
		return null;
	}
}

function refreshCoverFromOwners(
	jobType: JobType,
	fileList: FileListInfo | null,
	selectedFiles: ReadonlyArray<AudioFile>,
	cover: CoverUiState,
): CoverUiState {
	const displayPath = resolveCoverDisplayPath(jobType, fileList, [...selectedFiles]);
	if (!displayPath) {
		return displayCover(cover, null);
	}
	return displayCover(cover, effectiveCoverForFile(displayPath));
}

function commitCoverToOwners(
	jobType: JobType,
	fileList: FileListInfo | null,
	selectedFiles: ReadonlyArray<AudioFile>,
	coverArtBytes: number[] | null,
	markRemoval: boolean,
): boolean {
	const ownerPaths = resolveCoverOwnerPaths(jobType, fileList, [...selectedFiles]);
	if (ownerPaths.length === 0) {
		return false;
	}
	const intentPatch =
		markRemoval || !coverArtBytes || coverArtBytes.length === 0
			? { cover_art: { op: 'clear' as const } }
			: { cover_art: { op: 'set' as const, value: [...coverArtBytes] } };
	for (const filePath of ownerPaths) {
		stageMetadataIntentPatch(filePath, intentPatch);
	}
	return true;
}

export const hydrateMetadataSelectionAtom = Atom.fn((activeElement: Element | null, get) => {
	const capability = get(metadataCapabilityAtom);
	const session = get(inputSessionAtom);
	const jobType = get(jobTypeAtom);
	const editor = get(metadataEditorAtom);
	const selectedFiles = selectedFilesFromSession(session);
	const nextKey = selectionKeyFor(selectedFiles);
	const committed = commitFocusedControlValue(editor.form, activeElement);
	let nextEditor: MetadataEditorState = {
		...editor,
		form: committed.form,
		focusedFieldId: committed.focusedFieldId,
		formRevision: committed.form === editor.form ? editor.formRevision : editor.formRevision + 1,
	};

	const files = session.fileList?.files ?? [];
	if (files.length === 0) {
		clearMetadataSession();
		nextEditor = {
			...emptyEditor(),
			hydrateRequestId: editor.hydrateRequestId + 1,
		};
		get.set(metadataEditorAtom, nextEditor);
		return Effect.void;
	}
	syncRemovedFiles(files);

	if (nextKey === editor.selectionKey) {
		nextEditor = bumpCover(
			nextEditor,
			refreshCoverFromOwners(jobType, session.fileList, selectedFiles, nextEditor.cover),
		);
		get.set(metadataEditorAtom, nextEditor);
		return autoLoadCoverIfNeeded(
			capability,
			session.fileList,
			selectedFiles,
			jobType,
			nextEditor.hydrateRequestId,
			get as MetadataAtomGet,
		);
	}

	return Effect.gen(function* () {
		if (nextKey !== editor.selectionKey && editor.boundFiles.length > 0) {
			const prepared = yield* Effect.tryPromise({
				try: () =>
					prepareMetadataDrafts({
						form: nextEditor.form,
						cover: nextEditor.cover,
						selectedFiles: editor.boundFiles,
						validate: (patch) => capability.validateMetadataIntentPatch(patch),
						readUncachedMetadata: (file) =>
							readUncachedMetadataSnapshot(file, (path) => capability.readAudioMetadata(path)),
					}),
				catch: (cause) => cause,
			}).pipe(
				Effect.catch(() =>
					Effect.succeed({ ok: true as const, prepared: { kind: 'none' as const } }),
				),
			);
			if (prepared.ok) {
				commitPreparedMetadataDrafts(prepared.prepared);
				if (prepared.prepared.kind !== 'none') {
					nextEditor = bumpForm(nextEditor, resetDirtyState(nextEditor.form));
					nextEditor = bumpCover(nextEditor, {
						hasCustomCoverArt: false,
						coverArtRemovalRequested: false,
					});
				}
			} else {
				nextEditor = bumpForm(
					nextEditor,
					applyMetadataFormValidationWarnings(
						nextEditor.form,
						readMetadataForm(nextEditor.form, {
							coverArtBytes: nextEditor.cover.currentCoverArt,
							coverArtRemovalRequested: nextEditor.cover.coverArtRemovalRequested,
						}),
						{ byField: {} },
					),
				);
				nextEditor = { ...nextEditor, statusMessage: prepared.message };
				get.set(metadataEditorAtom, nextEditor);
				return;
			}
		}

		const requestId = editor.hydrateRequestId + 1;
		nextEditor = {
			...nextEditor,
			hydrateRequestId: requestId,
			selectionKey: nextKey,
			boundFiles: selectedFiles,
			statusMessage: '',
		};
		get.set(metadataEditorAtom, nextEditor);

		if (selectedFiles.length === 0) {
			nextEditor = bumpForm(nextEditor, populateMetadataFormSingle({}));
			nextEditor = bumpCover(nextEditor, displayCover(createEmptyCoverUiState(), null));
			get.set(metadataEditorAtom, nextEditor);
			return;
		}

		const metadataList = yield* Effect.tryPromise({
			try: () => Promise.all(selectedFiles.map((file) => loadMetadataForFile(capability, file))),
			catch: (cause) => cause,
		}).pipe(Effect.catch(() => Effect.succeed(selectedFiles.map(() => null))));

		if (get(metadataEditorAtom).hydrateRequestId !== requestId) {
			return;
		}

		if (selectedFiles.length === 1) {
			nextEditor = bumpForm(nextEditor, populateMetadataFormSingle(metadataList[0] ?? {}));
		} else {
			nextEditor = bumpForm(
				nextEditor,
				populateMetadataFormMulti(
					metadataList.map((metadata) => metadata ?? {}),
					selectedFiles.length,
				),
			);
		}
		nextEditor = bumpCover(
			nextEditor,
			refreshCoverFromOwners(jobType, session.fileList, selectedFiles, {
				...nextEditor.cover,
				hasCustomCoverArt: false,
				coverArtRemovalRequested: false,
			}),
		);
		get.set(metadataEditorAtom, nextEditor);

		const focused = nextEditor.focusedFieldId;
		if (focused) {
			requestAnimationFrame(() => {
				document.getElementById(focused)?.focus();
			});
		}

		yield* autoLoadCoverIfNeeded(
			capability,
			session.fileList,
			selectedFiles,
			jobType,
			requestId,
			get as MetadataAtomGet,
		);
	});
}).pipe(Atom.keepAlive);

type MetadataAtomGet = {
	(atom: typeof metadataEditorAtom): MetadataEditorState;
	(atom: typeof inputSessionAtom): InputSessionState;
	(atom: typeof jobTypeAtom): JobType;
	readonly set: (atom: typeof metadataEditorAtom, value: MetadataEditorState) => void;
};

function autoLoadCoverIfNeeded(
	capability: MetadataCapability,
	fileList: FileListInfo | null,
	selectedFiles: ReadonlyArray<AudioFile>,
	jobType: JobType,
	hydrateRequestId: number,
	get: MetadataAtomGet,
) {
	return Effect.gen(function* () {
		const editor = get(metadataEditorAtom);
		const autoCoverRequestId = editor.autoCoverRequestId + 1;
		get.set(metadataEditorAtom, { ...editor, autoCoverRequestId });
		const firstValid = fileList?.files.find((file) => file.isValid);
		const selectedValid = selectedFiles.find((file) => file.isValid);
		const targetPath =
			jobType === 'merge' ? firstValid?.path : (selectedValid?.path ?? firstValid?.path);
		if (!targetPath) {
			return;
		}
		if (
			effectiveCoverForFile(targetPath) !== null ||
			getMetadataIntentPatchForFile(targetPath)?.cover_art
		) {
			return;
		}
		const metadata = yield* Effect.tryPromise({
			try: () => capability.readAudioMetadata(targetPath),
			catch: (cause) => cause,
		}).pipe(Effect.catch(() => Effect.succeed(null)));
		const current = get(metadataEditorAtom);
		if (
			!metadata ||
			current.hydrateRequestId !== hydrateRequestId ||
			current.autoCoverRequestId !== autoCoverRequestId ||
			current.cover.hasCustomCoverArt ||
			getMetadataIntentPatchForFile(targetPath)?.cover_art
		) {
			return;
		}
		const existing = getMetadataForFile(targetPath) ?? {};
		cacheMetadataForFile(targetPath, {
			...metadata,
			...existing,
			cover_art: metadata.cover_art || existing.cover_art,
		});
		const session = get(inputSessionAtom);
		const selected = selectedFilesFromSession(session);
		get.set(
			metadataEditorAtom,
			bumpCover(
				current,
				refreshCoverFromOwners(get(jobTypeAtom), session.fileList, selected, current.cover),
			),
		);
	});
}

export const setMetadataFieldValueAtom = Atom.fnSync(
	(command: { readonly inputId: string; readonly value: string }, get) => {
		const definition = getMetadataFieldDefinitionByInputId(command.inputId);
		if (!definition) return;
		const editor = get(metadataEditorAtom);
		const withValue = replaceField(editor.form, definition.inputId, { value: command.value });
		get.set(metadataEditorAtom, bumpForm(editor, applyFieldInput(withValue, definition.inputId)));
	},
).pipe(Atom.keepAlive);

export const setMetadataFieldActionAtom = Atom.fnSync(
	(command: { readonly actionId: string; readonly action: 'keep' | 'blank' }, get) => {
		const definition = getMetadataFieldDefinitionByActionId(command.actionId);
		if (!definition) return;
		const editor = get(metadataEditorAtom);
		get.set(
			metadataEditorAtom,
			bumpForm(editor, applyFieldAction(editor.form, definition.inputId, command.action)),
		);
	},
).pipe(Atom.keepAlive);

export const setCoverHoveredAtom = Atom.fnSync((hovered: boolean, get) => {
	const editor = get(metadataEditorAtom);
	get.set(metadataEditorAtom, { ...editor, cover: { ...editor.cover, isHovered: hovered } });
}).pipe(Atom.keepAlive);

export const setCoverDragOverAtom = Atom.fnSync((dragOver: boolean, get) => {
	const editor = get(metadataEditorAtom);
	get.set(metadataEditorAtom, { ...editor, cover: { ...editor.cover, isDragOver: dragOver } });
}).pipe(Atom.keepAlive);

export const setCoverUrlInputAtom = Atom.fnSync((value: string, get) => {
	const editor = get(metadataEditorAtom);
	get.set(metadataEditorAtom, { ...editor, cover: { ...editor.cover, urlInputValue: value } });
}).pipe(Atom.keepAlive);

function applyLoadedCoverArt(get: MetadataAtomGet, bytes: number[]): void {
	const editor = get(metadataEditorAtom);
	const session = get(inputSessionAtom);
	const selected = selectedFilesFromSession(session);
	const committed = commitCoverToOwners(get(jobTypeAtom), session.fileList, selected, bytes, false);
	if (!committed) {
		get.set(
			metadataEditorAtom,
			bumpCover(
				editor,
				refreshCoverFromOwners(get(jobTypeAtom), session.fileList, selected, editor.cover),
			),
		);
		return;
	}
	get.set(
		metadataEditorAtom,
		bumpCover(editor, {
			...displayCover(editor.cover, bytes),
			hasCustomCoverArt: true,
			coverArtRemovalRequested: false,
		}),
	);
}

export const setCustomCoverArtAtom = Atom.fnSync((coverArtBytes: number[] | null, get) => {
	if (!coverArtBytes || coverArtBytes.length === 0) {
		return;
	}
	applyLoadedCoverArt(get as MetadataAtomGet, coverArtBytes);
}).pipe(Atom.keepAlive);

export const clearCoverArtAtom = Atom.fnSync((_: undefined, get) => {
	const editor = get(metadataEditorAtom);
	const session = get(inputSessionAtom);
	const selected = selectedFilesFromSession(session);
	commitCoverToOwners(get(jobTypeAtom), session.fileList, selected, null, true);
	get.set(
		metadataEditorAtom,
		bumpCover(editor, {
			...displayCover(createEmptyCoverUiState(), null),
			coverArtRemovalRequested: true,
			hasCustomCoverArt: false,
			urlInputValue: '',
			message: { kind: 'hidden' },
		}),
	);
}).pipe(Atom.keepAlive);

export const loadCoverArtFromPickerAtom = Atom.fn((_: undefined, get) => {
	const capability = get(metadataCapabilityAtom);
	return Effect.tryPromise({
		try: async () => {
			const selectedFile = await capability.openFile({
				title: 'Select Cover Art Image',
				filters: [{ name: 'Image Files', extensions: [...COVER_ART_IMAGE_EXTENSION_HINTS] }],
			});
			if (!selectedFile) return;
			const imageData = await capability.loadCoverArtFile(selectedFile);
			applyLoadedCoverArt(get as MetadataAtomGet, imageData);
		},
		catch: (cause) => cause,
	}).pipe(
		Effect.catch((error) =>
			Effect.sync(() => {
				console.error('Failed to open file dialog:', error);
			}),
		),
	);
}).pipe(Atom.keepAlive);

export const loadCoverArtFromUrlAtom = Atom.fn((rawInput: string, get) => {
	const capability = get(metadataCapabilityAtom);
	const raw = rawInput.trim();
	return Effect.gen(function* () {
		const editor = get(metadataEditorAtom);
		if (!raw) {
			get.set(
				metadataEditorAtom,
				bumpCover(editor, { message: { kind: 'error', text: 'Paste an image URL first.' } }),
			);
			return null;
		}
		const parsed = parseCoverArtUrl(raw);
		if (!parsed) {
			get.set(
				metadataEditorAtom,
				bumpCover(editor, { message: { kind: 'error', text: 'Invalid URL format.' } }),
			);
			return null;
		}
		if (parsed.protocol !== 'https:') {
			get.set(
				metadataEditorAtom,
				bumpCover(editor, { message: { kind: 'error', text: 'Only HTTPS URLs are supported.' } }),
			);
			return null;
		}
		const normalized = parsed.toString();
		get.set(
			metadataEditorAtom,
			bumpCover(editor, {
				urlInputValue: normalized,
				isLoading: true,
				message: { kind: 'hidden' },
			}),
		);
		const imageData = yield* Effect.tryPromise({
			try: () => capability.loadCoverArtFromUrl(normalized),
			catch: (cause) => cause,
		}).pipe(
			Effect.catch((error) =>
				Effect.sync(() => {
					const message = formatCoverArtError(
						toUserMessage(error, { fallback: 'Unable to load image.' }),
						'Unable to load image.',
					);
					const current = get(metadataEditorAtom);
					get.set(
						metadataEditorAtom,
						bumpCover(current, {
							isLoading: false,
							message: { kind: 'error', text: message },
						}),
					);
					scheduleCoverMessageClear((cover) => {
						const latest = get(metadataEditorAtom);
						get.set(metadataEditorAtom, bumpCover(latest, cover));
					});
					return null;
				}),
			),
		);
		if (!imageData) {
			return null;
		}
		applyLoadedCoverArt(get as MetadataAtomGet, imageData);
		const current = get(metadataEditorAtom);
		get.set(
			metadataEditorAtom,
			bumpCover(current, {
				isLoading: false,
				message: { kind: 'success', text: 'Cover art loaded from URL.' },
			}),
		);
		scheduleCoverMessageClear((cover) => {
			const latest = get(metadataEditorAtom);
			get.set(metadataEditorAtom, bumpCover(latest, cover));
		});
		return normalized;
	});
}).pipe(Atom.keepAlive);

export const applyCoverArtDropAtom = Atom.fn((paths: ReadonlyArray<string>, get) => {
	const capability = get(metadataCapabilityAtom);
	const imageFile = paths.find((path) => COVER_ART_IMAGE_EXTENSION_HINT_PATTERN.test(path));
	if (!imageFile) {
		return Effect.succeed(false);
	}
	return Effect.tryPromise({
		try: async () => {
			const imageData = await capability.loadCoverArtFile(imageFile);
			applyLoadedCoverArt(get as MetadataAtomGet, imageData);
			return true;
		},
		catch: (cause) => cause,
	}).pipe(
		Effect.catch((error) =>
			Effect.sync(() => {
				console.error('Failed to load cover art file:', error);
				return false;
			}),
		),
	);
}).pipe(Atom.keepAlive);

export const applyLookupMetadataAtom = Atom.fnSync((metadata: Partial<AudiobookMetadata>, get) => {
	const editor = get(metadataEditorAtom);
	get.set(
		metadataEditorAtom,
		bumpForm(editor, applyMetadataToForm(editor.form, metadata, { mode: editor.form.mode })),
	);
}).pipe(Atom.keepAlive);

export const saveMetadataAtom = Atom.fn((_: undefined, get) => {
	const capability = get(metadataCapabilityAtom);
	const session = get(inputSessionAtom);
	const editor = get(metadataEditorAtom);
	if (!session.fileList?.files.length) {
		console.log('No files loaded - nothing to save');
		return Effect.void;
	}
	if (editor.saveInProgress) {
		get.set(metadataEditorAtom, { ...editor, statusMessage: 'Save already in progress...' });
		return Effect.void;
	}
	const committed = commitFocusedControlValue(editor.form, document.activeElement);
	let working: MetadataEditorState = {
		...editor,
		form: committed.form,
		focusedFieldId: committed.focusedFieldId,
		saveInProgress: true,
		statusMessage: 'Preparing metadata save...',
		formRevision: committed.form === editor.form ? editor.formRevision : editor.formRevision + 1,
	};
	get.set(metadataEditorAtom, working);

	return Effect.gen(function* () {
		const prepared = yield* Effect.tryPromise({
			try: () =>
				prepareMetadataDrafts({
					form: working.form,
					cover: working.cover,
					selectedFiles: selectedFilesFromSession(session),
					validate: (patch) => capability.validateMetadataIntentPatch(patch),
					readUncachedMetadata: (file) =>
						readUncachedMetadataSnapshot(file, (path) => capability.readAudioMetadata(path)),
				}),
			catch: (cause) => cause,
		});
		if (!prepared.ok) {
			get.set(metadataEditorAtom, {
				...get(metadataEditorAtom),
				saveInProgress: false,
				statusMessage: 'Fix metadata validation errors before saving.',
			});
			return;
		}
		commitPreparedMetadataDrafts(prepared.prepared);
		working = bumpForm(get(metadataEditorAtom), resetDirtyState(get(metadataEditorAtom).form));
		get.set(metadataEditorAtom, working);

		const validPaths = new Set(
			(session.fileList?.files ?? []).filter((file) => file.isValid).map((file) => file.path),
		);
		const pendingEntries = getPendingMetadataIntentEntries().filter(([filePath]) =>
			validPaths.has(filePath),
		);
		if (pendingEntries.length === 0) {
			get.set(metadataEditorAtom, {
				...get(metadataEditorAtom),
				saveInProgress: false,
				statusMessage: 'No pending metadata changes',
			});
			return;
		}
		const result = yield* Effect.tryPromise({
			try: () =>
				capability.saveMetadataBatch(
					pendingEntries.map(([filePath, metadataIntent]) => ({
						filePath,
						metadataPatch: metadataIntent,
					})),
				),
			catch: (cause) => cause,
		});
		for (const entry of result.results) {
			if (entry.status === 'success') {
				clearPendingMetadataForFile(entry.filePath);
			} else if (entry.status === 'failed') {
				console.error(`Failed metadata save for ${entry.filePath}:`, entry.error ?? entry.message);
			}
		}
		get.set(metadataEditorAtom, {
			...get(metadataEditorAtom),
			saveInProgress: false,
			statusMessage: `Metadata save complete: success=${result.summary.succeeded}, failed=${result.summary.failed}, cancelled=${result.summary.cancelled}`,
			cover: {
				...get(metadataEditorAtom).cover,
				hasCustomCoverArt: false,
				coverArtRemovalRequested: false,
			},
		});
	}).pipe(
		Effect.catch((error) =>
			Effect.sync(() => {
				console.error('Failed to save metadata:', error);
				get.set(metadataEditorAtom, {
					...get(metadataEditorAtom),
					saveInProgress: false,
					statusMessage: 'Save failed - see console',
				});
			}),
		),
	);
}).pipe(Atom.keepAlive);

export function readEditorHasDirtyMetadata(editor: MetadataEditorState): boolean {
	return hasDirtyMetadataFields(editor.form, editor.cover);
}

export function readEditorMetadata(editor: MetadataEditorState): Partial<AudiobookMetadata> {
	return readMetadataForm(editor.form, {
		coverArtBytes: editor.cover.currentCoverArt,
		coverArtRemovalRequested: editor.cover.coverArtRemovalRequested,
	});
}

export { buildMetadataDraftIntent };
