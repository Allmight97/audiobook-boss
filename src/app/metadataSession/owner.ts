import { createSignal, type Accessor } from 'solid-js';
import type { AudioFile, FileListInfo, JobType } from '../../types/audio';
import type { AudiobookMetadata } from '../../types/metadata';
import type { MetadataIntentPatch } from '../../types/metadataIntent';
import { coverArtBytesToDataUrl } from '../../lib/media/coverArtDataUrl';
import { toUserMessage } from '../../lib/tauri/appError';
import {
	liveMetadataCapability,
	type MetadataCapability,
} from '../../lib/tauri/capabilities/metadata';
import type { InputOwner } from '../inputSession';
import { createMetadataCache, isUsableMetadataCache, type MetadataStageResult } from './cache';
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
import type { MetadataDraftValidation } from './validation';

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
	loadCoverArtFromUrl(rawInput: string): Promise<string | null>;
	applyCoverArtDrop(paths: ReadonlyArray<string>): Promise<boolean>;
	applyLookupMetadata(
		file: AudioFile,
		metadata: Partial<AudiobookMetadata>,
		coverArtBytes?: number[],
	): boolean;
	applyDraftValidation(validation: MetadataDraftValidation): void;
	stageCurrentSelectionForProcess(): Promise<boolean>;
	save(): Promise<void>;
	readHasDirtyMetadata(): boolean;
	readMetadata(): Partial<AudiobookMetadata>;
	readCached(filePath: string): Partial<AudiobookMetadata> | undefined;
	stageIntent(filePath: string, patch: MetadataIntentPatch): MetadataStageResult;
	intentsForProcess(
		filePaths: readonly string[],
	): Promise<Record<string, MetadataIntentPatch> | null>;
	reset(): void;
};

export type MetadataOwnerDeps = {
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

type CoverLoadContext = {
	readonly generation: number;
	readonly selectionKey: string;
	readonly hydrateRequestId: number;
};

function toView(editor: MetadataEditorState): MetadataView {
	return {
		form: editor.form,
		cover: editor.cover,
		tags: projectTagPreviewValues(readMetadataFormPreviewValues(editor.form)),
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

	function commit(next: MetadataEditorState): void {
		editor = next;
		bump((n) => n + 1);
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

	function syncRemovedFiles(sessionFiles: ReadonlyArray<AudioFile>): void {
		cache.dropRemovedPaths(new Set(sessionFiles.map((file) => file.path)));
	}

	async function loadMetadataForFile(file: AudioFile): Promise<Partial<AudiobookMetadata> | null> {
		if (!file.isValid) return null;
		const started = generation;
		const existing = cache.getMetadataForFile(file.path);
		if (isUsableMetadataCache(existing)) return existing;
		try {
			const metadata = await capability().readAudioMetadata(file.path);
			if (generation !== started) return null;
			const latest = cache.getMetadataForFile(file.path);
			if (isUsableMetadataCache(latest)) return latest;
			cache.cacheMetadataForFile(file.path, metadata);
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
		const displayPath = resolveCoverDisplayPath(jobType, fileList, [...selectedFiles], cache);
		if (!displayPath) {
			return displayCover(cover, null);
		}
		return displayCover(cover, effectiveCoverForFile(displayPath, cache));
	}

	function commitCoverToOwners(coverArtBytes: number[] | null, markRemoval: boolean): boolean {
		const session = deps.input.session();
		const selected = selectedFilesFromSession(session);
		const ownerPaths = resolveCoverOwnerPaths(deps.input.jobType(), session.fileList, [
			...selected,
		]);
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
			commit(
				bumpCover(
					current,
					refreshCoverFromOwners(deps.input.jobType(), session.fileList, selected, current.cover),
				),
			);
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

	async function persistBoundDrafts(
		current: MetadataEditorState,
		signal?: AbortSignal,
	): Promise<{ readonly ok: true } | { readonly ok: false; readonly message: string }> {
		const started = generation;
		if (current.boundFiles.length === 0) {
			return { ok: true };
		}
		let prepared: Awaited<ReturnType<typeof prepareMetadataDrafts>>;
		try {
			prepared = await prepareMetadataDrafts({
				form: current.form,
				cover: current.cover,
				selectedFiles: current.boundFiles,
				validate: (patch) => capability().validateMetadataIntentPatch(patch),
				readUncachedMetadata: (file) =>
					readUncachedMetadataSnapshot(file, (path) => capability().readAudioMetadata(path), cache),
			});
		} catch {
			return { ok: false, message: 'Failed to validate metadata before changing selection.' };
		}
		if (!prepared.ok) {
			return { ok: false, message: prepared.message };
		}
		if (
			signal?.aborted ||
			generation !== started ||
			editor.selectionKey !== current.selectionKey ||
			editor.hydrateRequestId !== current.hydrateRequestId ||
			editor.formRevision !== current.formRevision ||
			editor.coverRevision !== current.coverRevision
		) {
			return {
				ok: false,
				message: 'Metadata changed during validation. Try changing selection again.',
			};
		}
		commitPreparedMetadataDrafts(prepared.prepared, cache);
		if (prepared.prepared.kind !== 'none') {
			commit(
				bumpCover(bumpForm(editor, resetDirtyState(editor.form)), {
					hasCustomCoverArt: false,
					coverArtRemovalRequested: false,
				}),
			);
		}
		return { ok: true };
	}

	function applyValidationFailure(message: string): void {
		const current = editor;
		commit({
			...bumpForm(
				current,
				applyMetadataFormValidationWarnings(
					current.form,
					readMetadataForm(current.form, {
						coverArtBytes: current.cover.currentCoverArt,
						coverArtRemovalRequested: current.cover.coverArtRemovalRequested,
					}),
					{ byField: {} },
				),
			),
			statusMessage: message,
		});
	}

	async function autoLoadCoverIfNeeded(
		fileList: FileListInfo | null,
		selectedFiles: ReadonlyArray<AudioFile>,
		jobType: JobType,
		hydrateRequestId: number,
	): Promise<void> {
		const started = generation;
		const current = editor;
		const autoCoverRequestId = current.autoCoverRequestId + 1;
		commit({ ...current, autoCoverRequestId });
		const firstValid = fileList?.files.find((file) => file.isValid);
		const selectedValid = selectedFiles.find((file) => file.isValid);
		const targetPath =
			jobType === 'merge' ? firstValid?.path : (selectedValid?.path ?? firstValid?.path);
		if (!targetPath) {
			return;
		}
		if (
			effectiveCoverForFile(targetPath, cache) !== null ||
			cache.getMetadataIntentPatchForFile(targetPath)?.cover_art
		) {
			return;
		}
		let metadata: Partial<AudiobookMetadata> | null;
		try {
			metadata = await capability().readAudioMetadata(targetPath);
		} catch (error) {
			if (generation !== started || editor.hydrateRequestId !== hydrateRequestId) {
				return;
			}
			surfaceCoverFailure(
				formatCoverArtError(
					toUserMessage(error, { fallback: 'Unable to load cover art.' }),
					'Unable to load cover art.',
				),
			);
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
		const existing = cache.getMetadataForFile(targetPath) ?? {};
		cache.cacheMetadataForFile(targetPath, {
			...metadata,
			...existing,
			cover_art: metadata.cover_art || existing.cover_art,
		});
		const session = deps.input.session();
		const selected = selectedFilesFromSession(session);
		commit(
			bumpCover(
				latest,
				refreshCoverFromOwners(deps.input.jobType(), session.fileList, selected, latest.cover),
			),
		);
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
			if (current.boundFiles.length === 0 || !hasDirtyMetadataFields(current.form, current.cover)) {
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
			const jobType = deps.input.jobType();
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

			const files = session.fileList?.files ?? [];
			if (files.length === 0) {
				generation += 1;
				cache.clear();
				commit({
					...emptyEditor(),
					hydrateRequestId: start.hydrateRequestId + 1,
				});
				return true;
			}
			syncRemovedFiles(files);

			if (nextKey === start.selectionKey) {
				next = bumpCover(
					next,
					refreshCoverFromOwners(jobType, session.fileList, selectedFiles, next.cover),
				);
				if (next !== start) {
					commit(next);
				}
				await autoLoadCoverIfNeeded(
					session.fileList,
					selectedFiles,
					jobType,
					start.hydrateRequestId,
				);
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
						fields[field.inputId] = current;
				}
				form = { ...form, fields };
			}
			next = bumpForm(latest, form);
			next = bumpCover(
				next,
				refreshCoverFromOwners(jobType, session.fileList, selectedFiles, next.cover),
			);
			commit(next);

			const focused = next.focusedFieldId;
			if (focused) {
				requestAnimationFrame(() => {
					document.getElementById(focused)?.focus();
				});
			}

			await autoLoadCoverIfNeeded(session.fileList, selectedFiles, jobType, requestId);
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
			try {
				const selectedFile = await capability().openFile({
					title: 'Select Cover Art Image',
					filters: [{ name: 'Image Files', extensions: [...COVER_ART_IMAGE_EXTENSION_HINTS] }],
				});
				if (!selectedFile) return;
				const imageData = await capability().loadCoverArtFile(selectedFile);
				applyLoadedCoverArt(imageData, loadContext);
			} catch (error) {
				if (!coverLoadStillValid(loadContext)) return;
				console.error('Failed to open file dialog:', error);
				surfaceCoverFailure(
					formatCoverArtError(
						toUserMessage(error, { fallback: 'Unable to load cover art.' }),
						'Unable to load cover art.',
					),
				);
			}
		},
		async loadCoverArtFromUrl(rawInput) {
			const raw = rawInput.trim();
			const current = editor;
			if (!raw) {
				commit(
					bumpCover(current, { message: { kind: 'error', text: 'Paste an image URL first.' } }),
				);
				return null;
			}
			const parsed = parseCoverArtUrl(raw);
			if (!parsed) {
				commit(bumpCover(editor, { message: { kind: 'error', text: 'Invalid URL format.' } }));
				return null;
			}
			if (parsed.protocol !== 'https:') {
				commit(
					bumpCover(editor, {
						message: { kind: 'error', text: 'Only HTTPS URLs are supported.' },
					}),
				);
				return null;
			}
			const normalized = parsed.toString();
			const loadContext = readCoverLoadContext(editor);
			commit(
				bumpCover(editor, {
					urlInputValue: normalized,
					isLoading: true,
					message: { kind: 'hidden' },
				}),
			);
			try {
				const imageData = await capability().loadCoverArtFromUrl(normalized);
				if (!coverLoadStillValid(loadContext)) return null;
				applyLoadedCoverArt(imageData, loadContext);
				commit(
					bumpCover(editor, {
						isLoading: false,
						message: { kind: 'success', text: 'Cover art loaded from URL.' },
					}),
				);
				scheduleCoverMessageClear();
				return normalized;
			} catch (error) {
				if (!coverLoadStillValid(loadContext)) return null;
				surfaceCoverFailure(
					formatCoverArtError(
						toUserMessage(error, { fallback: 'Unable to load image.' }),
						'Unable to load image.',
					),
				);
				return null;
			}
		},
		async applyCoverArtDrop(paths) {
			const imageFile = paths.find((path) => COVER_ART_IMAGE_EXTENSION_HINT_PATTERN.test(path));
			if (!imageFile) {
				return false;
			}
			const loadContext = readCoverLoadContext(editor);
			try {
				const imageData = await capability().loadCoverArtFile(imageFile);
				applyLoadedCoverArt(imageData, loadContext);
				return true;
			} catch (error) {
				if (!coverLoadStillValid(loadContext)) return false;
				console.error('Failed to load cover art file:', error);
				surfaceCoverFailure(
					formatCoverArtError(
						toUserMessage(error, { fallback: 'Unable to load cover art.' }),
						'Unable to load cover art.',
					),
				);
				return false;
			}
		},
		applyLookupMetadata(file, metadata, coverArtBytes) {
			const current = editor;
			const selected = selectedFilesFromSession(deps.input.session());
			const matches = (files: ReadonlyArray<AudioFile>) =>
				files.length === 1 && files[0].path === file.path && files[0].inputId === file.inputId;
			if (current.saveInProgress || !matches(current.boundFiles) || !matches(selected))
				return false;
			commit(
				bumpForm(
					current,
					applyMetadataToForm(current.form, metadata, { mode: 'single', markDirty: true }),
				),
			);
			if (coverArtBytes?.length) applyLoadedCoverArt(coverArtBytes);
			return true;
		},
		applyDraftValidation(validation) {
			// Diagnostics do not invalidate drafts captured by pending intents.
			const current = editor;
			const nextForm = applyMetadataFormValidationWarnings(
				current.form,
				readMetadataForm(current.form, {
					coverArtBytes: current.cover.currentCoverArt,
					coverArtRemovalRequested: current.cover.coverArtRemovalRequested,
				}),
				{
					byField: {
						series_part: validation.errors.byField.series_part,
						subseries_part: validation.errors.byField.subseries_part,
					},
				},
			);
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
		async stageCurrentSelectionForProcess() {
			const started = generation;
			const start = editor;
			const captured = {
				selectionKey: start.selectionKey,
				formRevision: start.formRevision,
				coverRevision: start.coverRevision,
			};
			const session = deps.input.session();
			const selectedFiles = selectedFilesFromSession(session);
			let prepared: Awaited<ReturnType<typeof prepareMetadataDrafts>>;
			try {
				prepared = await prepareMetadataDrafts({
					form: start.form,
					cover: start.cover,
					selectedFiles,
					validate: (patch) => capability().validateMetadataIntentPatch(patch),
					readUncachedMetadata: (file) =>
						readUncachedMetadataSnapshot(
							file,
							(path) => capability().readAudioMetadata(path),
							cache,
						),
				});
			} catch {
				return false;
			}
			if (generation !== started) return false;
			if (!prepared.ok) {
				applyValidationFailure(prepared.message);
				return false;
			}
			const latest = editor;
			if (
				generation !== started ||
				latest.selectionKey !== captured.selectionKey ||
				latest.formRevision !== captured.formRevision ||
				latest.coverRevision !== captured.coverRevision
			) {
				return false;
			}
			return commitPreparedMetadataDrafts(prepared.prepared, cache);
		},
		async save() {
			const started = generation;
			const session = deps.input.session();
			const current = editor;
			if (!session.fileList?.files.length) {
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
			const captured = editor;
			try {
				const prepared = await prepareMetadataDrafts({
					form: editor.form,
					cover: editor.cover,
					selectedFiles: selectedFilesFromSession(session),
					validate: (patch) => capability().validateMetadataIntentPatch(patch),
					readUncachedMetadata: (file) =>
						readUncachedMetadataSnapshot(
							file,
							(path) => capability().readAudioMetadata(path),
							cache,
						),
				});
				if (generation !== started) return;
				if (
					editor.selectionKey !== captured.selectionKey ||
					editor.formRevision !== captured.formRevision ||
					editor.coverRevision !== captured.coverRevision
				) {
					commit({
						...editor,
						saveInProgress: false,
						statusMessage:
							'Metadata changed during validation. Save again to include the latest edits.',
					});
					return;
				}
				if (!prepared.ok) {
					commit({
						...editor,
						saveInProgress: false,
						statusMessage: 'Fix metadata validation errors before saving.',
					});
					return;
				}
				commitPreparedMetadataDrafts(prepared.prepared, cache);
				commit(bumpForm(editor, resetDirtyState(editor.form)));

				const validPaths = new Set(
					(session.fileList?.files ?? []).filter((file) => file.isValid).map((file) => file.path),
				);
				const pendingEntries = cache
					.getPendingMetadataIntentEntries()
					.filter(([filePath]) => validPaths.has(filePath));
				if (pendingEntries.length === 0) {
					commit({
						...editor,
						saveInProgress: false,
						statusMessage: 'No pending metadata changes',
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
						if (
							cache.getMetadataIntentPatchForFile(entry.filePath) === submitted.get(entry.filePath)
						)
							cache.clearPendingMetadataForFile(entry.filePath);
					} else if (entry.status === 'failed') {
						console.error(
							`Failed metadata save for ${entry.filePath}:`,
							entry.error ?? entry.message,
						);
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
					statusMessage: 'Save failed - see console',
				});
			}
		},
		readHasDirtyMetadata() {
			const current = editor;
			return hasDirtyMetadataFields(current.form, current.cover);
		},
		readMetadata() {
			const current = editor;
			return readMetadataForm(current.form, {
				coverArtBytes: current.cover.currentCoverArt,
				coverArtRemovalRequested: current.cover.coverArtRemovalRequested,
			});
		},
		readCached(filePath) {
			return cache.getMetadataForFile(filePath);
		},
		stageIntent(filePath, patch) {
			return cache.stageMetadataIntentPatch(filePath, patch);
		},
		async intentsForProcess(filePaths) {
			const started = generation;
			await Promise.all(
				filePaths.map(async (filePath) => {
					if (isUsableMetadataCache(cache.getMetadataForFile(filePath))) {
						return;
					}
					try {
						const metadata = await capability().readAudioMetadata(filePath);
						if (generation !== started) return;
						cache.cacheMetadataForFile(filePath, metadata);
					} catch (error) {
						console.warn('Failed to load metadata for batch file:', filePath, error);
					}
				}),
			);
			return generation === started ? cache.collectActionableMetadataIntent(filePaths) : null;
		},
		reset() {
			generation += 1;
			if (coverMessageTimeoutId !== null) {
				window.clearTimeout(coverMessageTimeoutId);
				coverMessageTimeoutId = null;
			}
			cache.clear();
			commit(emptyEditor());
		},
	};
}
