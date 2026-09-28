import { createSignal, type Accessor } from 'solid-js';
import type { AudioFile } from '../../types/audio';
import type { AudiobookMetadata } from '../../types/metadata';
import type { MetadataIntentPatch } from '../../types/metadataIntent';
import { coverArtBytesToDataUrl } from '../../lib/media/coverArtDataUrl';
import { isCancellation, toUserMessage } from '../../lib/tauri/appError';
import {
	liveMetadataCapability,
	type MetadataCapability,
} from '../../lib/tauri/capabilities/metadata';
import type { InputOwner } from '../inputSession';
import { createMetadataCache } from './cache';
import {
	COVER_ART_IMAGE_EXTENSION_HINTS,
	COVER_ART_IMAGE_EXTENSION_HINT_PATTERN,
	createEmptyCoverUiState,
	type CoverUiState,
} from './cover';
import {
	effectiveCoverForFile,
	resolveCoverDisplayPath,
	resolveCoverOwnerPaths,
} from './coverOwner';
import {
	getMetadataFieldDefinitionByActionId,
	METADATA_FIELD_DEFINITIONS,
	getMetadataFieldDefinitionByInputId,
	createEmptyFormState,
	replaceField,
	type MetadataFieldId,
	type MetadataFormState,
} from './fields';
import {
	applyFieldAction,
	applyFieldInput,
	applyLookupValues,
	applyMetadataFormValidationWarnings,
	commitFocusedControlValue,
	formValue,
	hasDirtyFields,
	populateMetadataFormMulti,
	populateMetadataFormSingle,
	resetDirtyState,
} from './form';
import {
	commitPreparedMetadataDrafts,
	prepareMetadataDrafts,
	readUncachedMetadataSnapshot,
	type PrepareMetadataDraftsResult,
} from './staging';
import { projectTagPreviewValues } from './tags';
import type { MetadataDraftValidation } from './validation';

type MetadataEditorState = {
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
	readonly albumSortPreview: string;
};

export type MetadataView = {
	readonly form: MetadataFormState;
	readonly cover: CoverUiState;
	readonly tags: ReturnType<typeof projectTagPreviewValues>;
	readonly saveInProgress: boolean;
	readonly focusedFieldId: MetadataFieldId | null;
	readonly statusMessage: string;
};

/** Outcome of staging the bound form's edits into the session cache. */
export type MetadataStageOutcome =
	| { readonly status: 'staged' }
	| { readonly status: 'invalid'; readonly message: string }
	| { readonly status: 'stale' }
	| { readonly status: 'noTarget' };

export type MetadataOwner = {
	readonly view: Accessor<MetadataView>;
	readonly capability: Accessor<MetadataCapability>;
	hydrateSelection(activeElement: Element | null): Promise<boolean>;
	canChangeSelection(signal?: AbortSignal): Promise<boolean>;
	setFieldValue(command: { readonly inputId: string; readonly value: string }): void;
	setFieldAction(command: { readonly actionId: string; readonly action: 'keep' | 'blank' }): void;
	setCoverHovered(hovered: boolean): void;
	setCoverDragOver(dragOver: boolean): void;
	setCoverUrlInput(value: string): void;
	setCustomCoverArt(coverArtBytes: number[] | null): void;
	clearCoverArt(): void;
	loadCoverArtFromPicker(): Promise<void>;
	loadCoverArtFromUrl(rawInput: string): Promise<void>;
	applyCoverArtDrop(paths: ReadonlyArray<string>): Promise<boolean>;
	applyLookupMetadata(
		file: AudioFile,
		metadata: Partial<AudiobookMetadata>,
		coverArtBytes?: number[],
	): boolean;
	applyDraftValidation(validation: MetadataDraftValidation): void;
	stageCurrentSelection(): Promise<MetadataStageOutcome>;
	save(): Promise<void>;
	readCached(filePath: string): Partial<AudiobookMetadata> | undefined;
	intentsForProcess(
		filePaths: readonly string[],
	): Promise<Record<string, MetadataIntentPatch> | null>;
	reset(): void;
};

type MetadataOwnerDeps = {
	readonly input: InputOwner;
	readonly capability?: MetadataCapability;
	readonly isForegroundProcessing?: () => boolean;
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
		albumSortPreview: '',
	};
}

function bumpForm(editor: MetadataEditorState, form: MetadataFormState): MetadataEditorState {
	return { ...editor, form, formRevision: editor.formRevision + 1 };
}

function bumpCover(editor: MetadataEditorState, cover: Partial<CoverUiState>): MetadataEditorState {
	const nextCover = { ...editor.cover, ...cover };
	const changed =
		nextCover.currentCoverArt !== editor.cover.currentCoverArt ||
		nextCover.hasCustomCoverArt !== editor.cover.hasCustomCoverArt ||
		nextCover.coverArtRemovalRequested !== editor.cover.coverArtRemovalRequested;
	return { ...editor, cover: nextCover, coverRevision: editor.coverRevision + (changed ? 1 : 0) };
}

function selectionKeyFor(files: ReadonlyArray<AudioFile>): string {
	return files
		.map((file) => file.path)
		.sort()
		.join('\0');
}

function selectedFilesFromSession(session: {
	readonly files: ReadonlyArray<AudioFile>;
	readonly selectedIndices: ReadonlyArray<number>;
}): AudioFile[] {
	const files = session.files;
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

/** Everything a draft preparation depends on; any change makes its result stale. */
type DraftSnapshot = {
	readonly generation: number;
	readonly selectionKey: string;
	readonly hydrateRequestId: number;
	readonly formRevision: number;
	readonly coverRevision: number;
};

type DraftPreparation =
	| PrepareMetadataDraftsResult
	| { readonly status: 'stale' }
	| { readonly status: 'failed'; readonly error: unknown };

type CoverLoadContext = {
	readonly generation: number;
	readonly selectionKey: string;
	readonly hydrateRequestId: number;
};

function toView(editor: MetadataEditorState): MetadataView {
	return {
		form: editor.form,
		cover: editor.cover,
		tags: projectTagPreviewValues(editor.form, editor.albumSortPreview),
		saveInProgress: editor.saveInProgress,
		focusedFieldId: editor.focusedFieldId,
		statusMessage: editor.statusMessage,
	};
}

export function createMetadataOwner(deps: MetadataOwnerDeps): MetadataOwner {
	const cache = createMetadataCache();
	let editor = emptyEditor();
	let generation = 0;
	const [rev, bump] = createSignal(0, { ownedWrite: true });
	const capabilityValue = deps.capability ?? liveMetadataCapability;
	const capability: Accessor<MetadataCapability> = () => capabilityValue;
	const view: Accessor<MetadataView> = () => {
		rev();
		return toView(editor);
	};
	const isForegroundProcessing = deps.isForegroundProcessing ?? (() => false);
	let coverMessageTimeoutId: number | null = null;

	function readCoverLoadContext(state: MetadataEditorState): CoverLoadContext {
		return {
			generation,
			selectionKey: state.selectionKey,
			hydrateRequestId: state.hydrateRequestId,
		};
	}

	function coverLoadStillValid(context: CoverLoadContext): boolean {
		return (
			generation === context.generation &&
			editor.selectionKey === context.selectionKey &&
			editor.hydrateRequestId === context.hydrateRequestId
		);
	}

	let albumSortRequest = { key: '', id: 0 };

	/** What Rust needs to project the album sort processing would write for the bound form. */
	function albumSortInput(state: MetadataEditorState): Partial<AudiobookMetadata> {
		const value = (id: MetadataFieldId) => formValue(state.form, id) || undefined;
		const single = state.boundFiles.length === 1 ? state.boundFiles[0] : undefined;
		return {
			title: value('meta-title'),
			series: value('meta-series'),
			series_part: value('meta-series-part'),
			album_sort: single ? cache.getMetadataForFile(single.path)?.album_sort : undefined,
		};
	}

	function refreshAlbumSortPreview(state: MetadataEditorState): void {
		const input = albumSortInput(state);
		const key = JSON.stringify(input);
		if (key === albumSortRequest.key) return;
		const id = albumSortRequest.id + 1;
		albumSortRequest = { key, id };
		// A preview failure is logged and never interrupts the edit that triggered it.
		Promise.resolve()
			.then(() => capability().previewAlbumSort(input))
			.then(
				(albumSort) => {
					if (albumSortRequest.id === id) commit({ ...editor, albumSortPreview: albumSort ?? '' });
				},
				(error: unknown) => {
					if (albumSortRequest.id === id) console.warn('Failed to preview album sort:', error);
				},
			);
	}

	function commit(next: MetadataEditorState): void {
		editor = next;
		bump((n) => n + 1);
		refreshAlbumSortPreview(next);
	}

	function scheduleCoverMessageClear(): void {
		if (coverMessageTimeoutId !== null) {
			window.clearTimeout(coverMessageTimeoutId);
		}
		coverMessageTimeoutId = window.setTimeout(() => {
			coverMessageTimeoutId = null;
			commit(bumpCover(editor, { message: { kind: 'hidden' } }));
		}, 4000);
	}

	function surfaceCoverFailure(message: string): void {
		commit(
			bumpCover(editor, {
				isLoading: false,
				message: { kind: 'error', text: message },
			}),
		);
		scheduleCoverMessageClear();
	}

	function failCoverLoad(error: unknown, context: CoverLoadContext, fallback: string): void {
		if (!coverLoadStillValid(context)) return;
		console.error('Failed to load cover art:', error);
		surfaceCoverFailure(toUserMessage(error, { fallback }));
	}

	async function loadCoverFromFile(path: string, context: CoverLoadContext): Promise<boolean> {
		try {
			applyLoadedCoverArt(await capability().loadCoverArtFile(path), context);
			return true;
		} catch (error) {
			failCoverLoad(error, context, 'Unable to load cover art.');
			return false;
		}
	}

	function syncRemovedFiles(sessionFiles: ReadonlyArray<AudioFile>): void {
		cache.dropRemovedPaths(new Set(sessionFiles.map((file) => file.path)));
	}

	async function loadMetadataForFile(file: AudioFile): Promise<Partial<AudiobookMetadata> | null> {
		if (!file.isValid) return null;
		const started = generation;
		if (cache.hasSourceMetadata(file.path)) return cache.getMetadataForFile(file.path) ?? null;
		try {
			const metadata = await capability().readAudioMetadata(file.path);
			if (generation !== started) return null;
			cache.recordSourceMetadata(file.path, metadata);
			return cache.getMetadataForFile(file.path) ?? null;
		} catch (error) {
			console.warn('Failed to load metadata:', error);
			return null;
		}
	}

	function refreshCoverFromOwners(
		files: ReadonlyArray<AudioFile>,
		selectedFiles: ReadonlyArray<AudioFile>,
		cover: CoverUiState,
	): CoverUiState {
		const displayPath = resolveCoverDisplayPath(files, [...selectedFiles], cache);
		if (!displayPath) {
			return displayCover(cover, null);
		}
		return displayCover(cover, effectiveCoverForFile(displayPath, cache));
	}

	function commitCoverToOwners(coverArtBytes: number[] | null, markRemoval: boolean): boolean {
		const session = deps.input.session();
		const selected = selectedFilesFromSession(session);
		const ownerPaths = resolveCoverOwnerPaths([...selected]);
		if (ownerPaths.length === 0) {
			return false;
		}
		const intentPatch =
			markRemoval || !coverArtBytes || coverArtBytes.length === 0
				? { cover_art: { op: 'clear' as const } }
				: { cover_art: { op: 'set' as const, value: [...coverArtBytes] } };
		for (const filePath of ownerPaths) {
			cache.stageMetadataIntentPatch(filePath, intentPatch);
		}
		return true;
	}

	function applyLoadedCoverArt(bytes: number[], loadContext?: CoverLoadContext): void {
		if (loadContext && !coverLoadStillValid(loadContext)) {
			return;
		}
		const current = editor;
		const session = deps.input.session();
		const selected = selectedFilesFromSession(session);
		if (!commitCoverToOwners(bytes, false)) {
			commit(bumpCover(current, refreshCoverFromOwners(session.files, selected, current.cover)));
			return;
		}
		commit(
			bumpCover(current, {
				...displayCover(current.cover, bytes),
				hasCustomCoverArt: true,
				coverArtRemovalRequested: false,
			}),
		);
	}

	function changedSince(snapshot: DraftSnapshot): boolean {
		return (
			generation !== snapshot.generation ||
			editor.selectionKey !== snapshot.selectionKey ||
			editor.hydrateRequestId !== snapshot.hydrateRequestId ||
			editor.formRevision !== snapshot.formRevision ||
			editor.coverRevision !== snapshot.coverRevision
		);
	}

	/** Prepares `state`'s form for `files`; the result is stale once anything it read changes. */
	async function prepareDraft(
		state: MetadataEditorState,
		files: ReadonlyArray<AudioFile>,
		signal?: AbortSignal,
	): Promise<DraftPreparation> {
		const snapshot: DraftSnapshot = {
			generation,
			selectionKey: state.selectionKey,
			hydrateRequestId: state.hydrateRequestId,
			formRevision: state.formRevision,
			coverRevision: state.coverRevision,
		};
		let result: DraftPreparation;
		try {
			result = await prepareMetadataDrafts({
				form: state.form,
				files,
				validate: (patch) => capability().validateMetadataIntentPatch(patch),
				readUncachedMetadata: (file) =>
					readUncachedMetadataSnapshot(file, (path) => capability().readAudioMetadata(path), cache),
			});
		} catch (error) {
			result = { status: 'failed', error };
		}
		return signal?.aborted || changedSince(snapshot) ? { status: 'stale' } : result;
	}

	async function persistBoundDrafts(
		current: MetadataEditorState,
		signal?: AbortSignal,
	): Promise<{ readonly ok: true } | { readonly ok: false; readonly message: string }> {
		if (current.boundFiles.length === 0) {
			return { ok: true };
		}
		const result = await prepareDraft(current, current.boundFiles, signal);
		switch (result.status) {
			case 'stale':
				return {
					ok: false,
					message: 'Metadata changed during validation. Try changing selection again.',
				};
			case 'failed':
				return { ok: false, message: 'Failed to validate metadata before changing selection.' };
			case 'invalid':
				return { ok: false, message: result.message };
			case 'noTarget':
				// Only invalid inputs are bound; they cannot carry metadata edits.
				return { ok: true };
			case 'ready':
				if (result.prepared) {
					commitPreparedMetadataDrafts(result.prepared, cache);
					commit(
						bumpCover(bumpForm(editor, resetDirtyState(editor.form)), {
							hasCustomCoverArt: false,
							coverArtRemovalRequested: false,
						}),
					);
				}
				return { ok: true };
		}
	}

	function applyValidationFailure(message: string): void {
		const current = editor;
		commit({
			...bumpForm(current, applyMetadataFormValidationWarnings(current.form, { byField: {} })),
			statusMessage: message,
		});
	}

	async function autoLoadCoverIfNeeded(
		files: ReadonlyArray<AudioFile>,
		selectedFiles: ReadonlyArray<AudioFile>,
		hydrateRequestId: number,
	): Promise<void> {
		const started = generation;
		const current = editor;
		const autoCoverRequestId = current.autoCoverRequestId + 1;
		commit({ ...current, autoCoverRequestId });
		const firstValid = files.find((file) => file.isValid);
		const selectedValid = selectedFiles.find((file) => file.isValid);
		const targetPath = selectedValid?.path ?? firstValid?.path;
		if (!targetPath) {
			return;
		}
		if (
			effectiveCoverForFile(targetPath, cache) !== null ||
			cache.getMetadataIntentPatchForFile(targetPath)?.cover_art ||
			cache.hasSourceMetadata(targetPath)
		) {
			// A cached read already answered whether this file has art.
			return;
		}
		let metadata: Partial<AudiobookMetadata> | null;
		try {
			metadata = await capability().readAudioMetadata(targetPath);
		} catch (error) {
			if (generation !== started || editor.hydrateRequestId !== hydrateRequestId) {
				return;
			}
			surfaceCoverFailure(toUserMessage(error, { fallback: 'Unable to load cover art.' }));
			return;
		}
		const latest = editor;
		if (
			generation !== started ||
			!metadata ||
			latest.hydrateRequestId !== hydrateRequestId ||
			latest.autoCoverRequestId !== autoCoverRequestId ||
			latest.cover.hasCustomCoverArt ||
			cache.getMetadataIntentPatchForFile(targetPath)?.cover_art
		) {
			return;
		}
		cache.recordSourceMetadata(targetPath, metadata);
		const session = deps.input.session();
		const selected = selectedFilesFromSession(session);
		commit(bumpCover(latest, refreshCoverFromOwners(session.files, selected, latest.cover)));
	}

	return {
		view,
		capability,
		async canChangeSelection(signal) {
			const started = generation;
			const current = editor;
			if (signal?.aborted || current.saveInProgress) {
				return false;
			}
			if (current.boundFiles.length === 0 || !hasDirtyFields(current.form)) {
				return true;
			}
			const persisted = await persistBoundDrafts(current, signal);
			if (signal?.aborted || generation !== started) return false;
			if (!persisted.ok) {
				applyValidationFailure(persisted.message);
				return false;
			}
			return true;
		},
		async hydrateSelection(activeElement) {
			const started = generation;
			const session = deps.input.session();
			const start = editor;
			const selectedFiles = selectedFilesFromSession(session);
			const nextKey = selectionKeyFor(selectedFiles);
			const committed = commitFocusedControlValue(start.form, activeElement);
			let next: MetadataEditorState = {
				...start,
				form: committed.form,
				focusedFieldId: committed.focusedFieldId,
				formRevision: committed.form === start.form ? start.formRevision : start.formRevision + 1,
			};

			const files = session.files;
			if (files.length === 0) {
				generation += 1;
				cache.clear();
				commit({
					...emptyEditor(),
					hydrateRequestId: start.hydrateRequestId + 1,
				});
				return true;
			}
			syncRemovedFiles(deps.input.view().sourceFiles);

			if (nextKey === start.selectionKey) {
				next = bumpCover(next, refreshCoverFromOwners(session.files, selectedFiles, next.cover));
				if (next !== start) {
					commit(next);
				}
				await autoLoadCoverIfNeeded(session.files, selectedFiles, start.hydrateRequestId);
				return generation === started && editor.selectionKey === nextKey;
			}

			const requestId = start.hydrateRequestId + 1;
			commit({ ...next, hydrateRequestId: requestId });
			next = editor;

			if (start.boundFiles.length > 0) {
				const persisted = await persistBoundDrafts({ ...next, boundFiles: start.boundFiles });
				if (generation !== started || editor.hydrateRequestId !== requestId) {
					return false;
				}
				if (!persisted.ok) {
					applyValidationFailure(persisted.message);
					return false;
				}
				next = editor;
			}

			next = {
				...bumpCover(next, createEmptyCoverUiState()),
				selectionKey: nextKey,
				boundFiles: selectedFiles,
				statusMessage: '',
			};
			commit(next);

			if (selectedFiles.length === 0) {
				next = bumpForm(next, populateMetadataFormSingle({}));
				next = bumpCover(next, displayCover(createEmptyCoverUiState(), null));
				commit(next);
				return true;
			}

			const loading = next;
			const metadataList = await Promise.all(
				selectedFiles.map((file) => loadMetadataForFile(file)),
			);
			if (generation !== started || editor.hydrateRequestId !== requestId) return false;
			const latest = editor;
			let form =
				selectedFiles.length === 1
					? populateMetadataFormSingle(metadataList[0] ?? {})
					: populateMetadataFormMulti(
							metadataList.map((metadata) => metadata ?? {}),
							selectedFiles.length,
						);
			if (latest.formRevision !== loading.formRevision) {
				const fields = { ...form.fields };
				for (const field of METADATA_FIELD_DEFINITIONS) {
					const current = latest.form.fields[field.inputId];
					const previous = loading.form.fields[field.inputId];
					if (
						current.dirty ||
						current.value !== previous.value ||
						current.action !== previous.action
					)
						fields[field.inputId] = { ...current, hydrated: form.fields[field.inputId].hydrated };
				}
				form = { ...form, fields };
			}
			next = bumpForm(latest, form);
			next = bumpCover(next, refreshCoverFromOwners(session.files, selectedFiles, next.cover));
			commit(next);

			const focused = next.focusedFieldId;
			if (focused) {
				requestAnimationFrame(() => {
					document.getElementById(focused)?.focus();
				});
			}

			await autoLoadCoverIfNeeded(session.files, selectedFiles, requestId);
			return generation === started && editor.hydrateRequestId === requestId;
		},
		setFieldValue(command) {
			const definition = getMetadataFieldDefinitionByInputId(command.inputId);
			if (!definition) return;
			const current = editor;
			const withValue = replaceField(current.form, definition.inputId, { value: command.value });
			commit(bumpForm(current, applyFieldInput(withValue, definition.inputId)));
		},
		setFieldAction(command) {
			const definition = getMetadataFieldDefinitionByActionId(command.actionId);
			if (!definition) return;
			const current = editor;
			commit(bumpForm(current, applyFieldAction(current.form, definition.inputId, command.action)));
		},
		setCoverHovered(hovered) {
			const current = editor;
			commit({ ...current, cover: { ...current.cover, isHovered: hovered } });
		},
		setCoverDragOver(dragOver) {
			const current = editor;
			commit({ ...current, cover: { ...current.cover, isDragOver: dragOver } });
		},
		setCoverUrlInput(value) {
			const current = editor;
			commit({ ...current, cover: { ...current.cover, urlInputValue: value } });
		},
		setCustomCoverArt(coverArtBytes) {
			if (!coverArtBytes || coverArtBytes.length === 0) {
				return;
			}
			applyLoadedCoverArt(coverArtBytes);
		},
		clearCoverArt() {
			const current = editor;
			commitCoverToOwners(null, true);
			commit(
				bumpCover(current, {
					...displayCover(createEmptyCoverUiState(), null),
					coverArtRemovalRequested: true,
					hasCustomCoverArt: false,
					urlInputValue: '',
					message: { kind: 'hidden' },
				}),
			);
		},
		async loadCoverArtFromPicker() {
			const loadContext = readCoverLoadContext(editor);
			let selectedFile: string | null;
			try {
				selectedFile = await capability().openFile({
					title: 'Select Cover Art Image',
					filters: [{ name: 'Image Files', extensions: [...COVER_ART_IMAGE_EXTENSION_HINTS] }],
				});
			} catch (error) {
				failCoverLoad(error, loadContext, 'Unable to open the image picker.');
				return;
			}
			if (selectedFile) await loadCoverFromFile(selectedFile, loadContext);
		},
		async loadCoverArtFromUrl(rawInput) {
			const url = rawInput.trim();
			if (!url) {
				commit(
					bumpCover(editor, { message: { kind: 'error', text: 'Paste an image URL first.' } }),
				);
				return;
			}
			const loadContext = readCoverLoadContext(editor);
			commit(
				bumpCover(editor, { urlInputValue: url, isLoading: true, message: { kind: 'hidden' } }),
			);
			try {
				const imageData = await capability().loadCoverArtFromUrl(url);
				if (!coverLoadStillValid(loadContext)) return;
				applyLoadedCoverArt(imageData, loadContext);
				commit(
					bumpCover(editor, {
						isLoading: false,
						message: { kind: 'success', text: 'Cover art loaded from URL.' },
					}),
				);
				scheduleCoverMessageClear();
			} catch (error) {
				failCoverLoad(error, loadContext, 'Unable to load image.');
			}
		},
		async applyCoverArtDrop(paths) {
			const imageFile = paths.find((path) => COVER_ART_IMAGE_EXTENSION_HINT_PATTERN.test(path));
			return imageFile ? loadCoverFromFile(imageFile, readCoverLoadContext(editor)) : false;
		},
		applyLookupMetadata(file, metadata, coverArtBytes) {
			const current = editor;
			const selected = selectedFilesFromSession(deps.input.session());
			const matches = (files: ReadonlyArray<AudioFile>) =>
				files.length === 1 && files[0].path === file.path && files[0].inputId === file.inputId;
			if (current.saveInProgress || !matches(current.boundFiles) || !matches(selected))
				return false;
			commit(bumpForm(current, applyLookupValues(current.form, metadata)));
			if (coverArtBytes?.length) applyLoadedCoverArt(coverArtBytes);
			return true;
		},
		applyDraftValidation(validation) {
			// Diagnostics do not invalidate drafts captured by pending intents.
			const current = editor;
			const nextForm = applyMetadataFormValidationWarnings(current.form, {
				byField: {
					series_part: validation.errors.byField.series_part,
					subseries_part: validation.errors.byField.subseries_part,
				},
			});
			if (nextForm === current.form && validation.ok) {
				return;
			}
			commit({
				...current,
				form: nextForm,
				statusMessage: validation.ok
					? current.statusMessage
					: (validation.errors.first ?? current.statusMessage),
			});
		},
		async stageCurrentSelection() {
			const current = editor;
			const result = await prepareDraft(current, current.boundFiles);
			switch (result.status) {
				case 'failed':
					throw result.error;
				case 'invalid':
					applyValidationFailure(result.message);
					return result;
				case 'stale':
				case 'noTarget':
					return result;
				case 'ready':
					if (result.prepared) {
						commitPreparedMetadataDrafts(result.prepared, cache);
						// Staged values become the baseline Keep restores.
						commit(bumpForm(editor, resetDirtyState(editor.form)));
					}
					return { status: 'staged' };
			}
		},
		async save() {
			const started = generation;
			const session = deps.input.session();
			const current = editor;
			if (!session.files.length) {
				console.log('No files loaded - nothing to save');
				return;
			}
			if (isForegroundProcessing()) {
				commit({ ...current, statusMessage: 'Cannot save metadata while a job is running.' });
				return;
			}
			if (current.saveInProgress) {
				commit({ ...current, statusMessage: 'Save already in progress...' });
				return;
			}
			const committed = commitFocusedControlValue(current.form, document.activeElement);
			commit({
				...current,
				form: committed.form,
				focusedFieldId: committed.focusedFieldId,
				saveInProgress: true,
				statusMessage: 'Preparing metadata save...',
				formRevision:
					committed.form === current.form ? current.formRevision : current.formRevision + 1,
			});
			try {
				const prepared = await prepareDraft(editor, editor.boundFiles);
				if (generation !== started) return;
				if (prepared.status === 'failed') throw prepared.error;
				if (prepared.status === 'stale' || prepared.status === 'invalid') {
					commit({
						...editor,
						saveInProgress: false,
						statusMessage:
							prepared.status === 'stale'
								? 'Metadata changed during validation. Save again to include the latest edits.'
								: 'Fix metadata validation errors before saving.',
					});
					return;
				}
				if (prepared.status === 'ready' && prepared.prepared) {
					commitPreparedMetadataDrafts(prepared.prepared, cache);
					commit(bumpForm(editor, resetDirtyState(editor.form)));
				}

				const validPaths = new Set(
					session.files
						.filter((file) => file.isValid && deps.input.sourcesFor(file).length === 1)
						.map((file) => file.path),
				);
				const pendingEntries = cache
					.getPendingMetadataIntentEntries()
					.filter(([filePath]) => validPaths.has(filePath));
				if (pendingEntries.length === 0) {
					commit({
						...editor,
						saveInProgress: false,
						statusMessage: deps.input
							.view()
							.files.some((file) => deps.input.sourcesFor(file).length > 1)
							? 'Title edits saved in this session. Process the title to write them to its merged audiobook.'
							: 'No pending metadata changes',
					});
					return;
				}
				const submitted = new Map(pendingEntries);
				const savedCoverRevision = editor.coverRevision;
				const result = await capability().saveMetadataBatch(
					pendingEntries.map(([filePath, metadataIntent]) => ({
						filePath,
						metadataPatch: metadataIntent,
					})),
				);
				if (generation !== started) return;
				for (const entry of result.results) {
					if (entry.status === 'success') {
						const saved = submitted.get(entry.filePath);
						if (saved) cache.commitSavedIntent(entry.filePath, saved);
					} else if (entry.status === 'failed') {
						console.error(`Failed metadata save for ${entry.filePath}; see Work Center.`);
					}
				}
				const latest = editor;
				commit({
					...latest,
					saveInProgress: false,
					statusMessage: `Metadata save complete: success=${result.summary.succeeded}, failed=${result.summary.failed}, cancelled=${result.summary.cancelled}`,
					cover:
						latest.coverRevision !== savedCoverRevision
							? latest.cover
							: {
									...latest.cover,
									hasCustomCoverArt: false,
									coverArtRemovalRequested: false,
								},
				});
			} catch (error) {
				if (generation !== started) return;
				console.error('Failed to save metadata:', error);
				commit({
					...editor,
					saveInProgress: false,
					statusMessage: isCancellation(error)
						? 'Metadata save cancelled.'
						: toUserMessage(error, { fallback: 'Metadata save failed.' }),
				});
			}
		},
		readCached(filePath) {
			return cache.getMetadataForFile(filePath);
		},
		// Rust reads each source's own tags during processing; only pending edits cross here.
		async intentsForProcess(filePaths) {
			return cache.collectActionableMetadataIntent(filePaths);
		},
		reset() {
			generation += 1;
			albumSortRequest = { key: '', id: albumSortRequest.id + 1 };
			if (coverMessageTimeoutId !== null) {
				window.clearTimeout(coverMessageTimeoutId);
				coverMessageTimeoutId = null;
			}
			cache.clear();
			commit(emptyEditor());
		},
	};
}
