import { createMemo, createSignal, type Accessor } from 'solid-js';
import type { AudioFile, FileListInfo, JobType } from '../../types/audio';
import type { AudiobookMetadata } from '../../types/metadata';
import { coverArtBytesToDataUrl } from '../../lib/media/coverArtDataUrl';
import { toUserMessage } from '../../lib/tauri/appError';
import {
	liveMetadataCapability,
	type MetadataCapability,
} from '../../lib/tauri/capabilities/metadata';
import type { InputOwner } from '../inputSession/owner';
import { getStatusView } from '../processing/view';
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
	hydrateSelection(activeElement: Element | null): Promise<void>;
	canChangeSelection(): Promise<boolean>;
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
	applyLookupMetadata(metadata: Partial<AudiobookMetadata>): void;
	applyDraftValidation(validation: MetadataDraftValidation): void;
	stageCurrentSelectionForProcess(): Promise<boolean>;
	save(): Promise<void>;
	readHasDirtyMetadata(): boolean;
	readMetadata(): Partial<AudiobookMetadata>;
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

type CoverLoadContext = {
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
	const [editor, setEditor] = createSignal(emptyEditor());
	const [capability] = createSignal(deps.capability ?? liveMetadataCapability);
	const view = createMemo(() => toView(editor()));
	const isForegroundProcessing =
		deps.isForegroundProcessing ?? (() => getStatusView().isProcessing);
	let coverMessageTimeoutId: number | null = null;

	function readCoverLoadContext(state: MetadataEditorState): CoverLoadContext {
		return {
			selectionKey: state.selectionKey,
			hydrateRequestId: state.hydrateRequestId,
		};
	}

	function coverLoadStillValid(context: CoverLoadContext): boolean {
		const latest = editor();
		return (
			latest.selectionKey === context.selectionKey &&
			latest.hydrateRequestId === context.hydrateRequestId
		);
	}

	function commit(next: MetadataEditorState): void {
		setEditor(next);
	}

	function scheduleCoverMessageClear(): void {
		if (coverMessageTimeoutId !== null) {
			window.clearTimeout(coverMessageTimeoutId);
		}
		coverMessageTimeoutId = window.setTimeout(() => {
			coverMessageTimeoutId = null;
			commit(bumpCover(editor(), { message: { kind: 'hidden' } }));
		}, 4000);
	}

	function surfaceCoverFailure(message: string): void {
		commit(
			bumpCover(editor(), {
				isLoading: false,
				message: { kind: 'error', text: message },
			}),
		);
		scheduleCoverMessageClear();
	}

	function syncRemovedFiles(sessionFiles: ReadonlyArray<AudioFile>): void {
		const livePaths = new Set(sessionFiles.map((file) => file.path));
		for (const [filePath] of getPendingMetadataIntentEntries()) {
			if (!livePaths.has(filePath)) {
				removeMetadataForFile(filePath);
			}
		}
	}

	async function loadMetadataForFile(file: AudioFile): Promise<Partial<AudiobookMetadata> | null> {
		if (!file.isValid) return null;
		const existing = getMetadataForFile(file.path);
		if (isUsableMetadataCache(existing)) return existing;
		try {
			const metadata = await capability().readAudioMetadata(file.path);
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
			stageMetadataIntentPatch(filePath, intentPatch);
		}
		return true;
	}

	function applyLoadedCoverArt(bytes: number[], loadContext?: CoverLoadContext): void {
		if (loadContext && !coverLoadStillValid(loadContext)) {
			return;
		}
		const current = editor();
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
	): Promise<{ readonly ok: true } | { readonly ok: false; readonly message: string }> {
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
					readUncachedMetadataSnapshot(file, (path) => capability().readAudioMetadata(path)),
			});
		} catch {
			return { ok: false, message: 'Failed to validate metadata before changing selection.' };
		}
		if (!prepared.ok) {
			return { ok: false, message: prepared.message };
		}
		commitPreparedMetadataDrafts(prepared.prepared);
		if (prepared.prepared.kind !== 'none') {
			commit(
				bumpCover(bumpForm(editor(), resetDirtyState(editor().form)), {
					hasCustomCoverArt: false,
					coverArtRemovalRequested: false,
				}),
			);
		}
		return { ok: true };
	}

	function applyValidationFailure(message: string): void {
		const current = editor();
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
		const current = editor();
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
			effectiveCoverForFile(targetPath) !== null ||
			getMetadataIntentPatchForFile(targetPath)?.cover_art
		) {
			return;
		}
		let metadata: Partial<AudiobookMetadata> | null;
		try {
			metadata = await capability().readAudioMetadata(targetPath);
		} catch (error) {
			if (editor().hydrateRequestId !== hydrateRequestId) {
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
		const latest = editor();
		if (
			!metadata ||
			latest.hydrateRequestId !== hydrateRequestId ||
			latest.autoCoverRequestId !== autoCoverRequestId ||
			latest.cover.hasCustomCoverArt ||
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
		async canChangeSelection() {
			const current = editor();
			if (current.saveInProgress) {
				return false;
			}
			if (current.boundFiles.length === 0 || !hasDirtyMetadataFields(current.form, current.cover)) {
				return true;
			}
			const persisted = await persistBoundDrafts(current);
			if (!persisted.ok) {
				applyValidationFailure(persisted.message);
				return false;
			}
			return true;
		},
		async hydrateSelection(activeElement) {
			const session = deps.input.session();
			const jobType = deps.input.jobType();
			const start = editor();
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
				clearMetadataSession();
				commit({
					...emptyEditor(),
					hydrateRequestId: start.hydrateRequestId + 1,
				});
				return;
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
				return;
			}

			const requestId = start.hydrateRequestId + 1;
			commit({ ...next, hydrateRequestId: requestId });
			next = editor();

			if (start.boundFiles.length > 0) {
				const persisted = await persistBoundDrafts({ ...next, boundFiles: start.boundFiles });
				if (editor().hydrateRequestId !== requestId) {
					return;
				}
				if (!persisted.ok) {
					applyValidationFailure(persisted.message);
					return;
				}
				next = editor();
			}

			next = {
				...next,
				selectionKey: nextKey,
				boundFiles: selectedFiles,
				statusMessage: '',
			};
			commit(next);

			if (selectedFiles.length === 0) {
				next = bumpForm(next, populateMetadataFormSingle({}));
				next = bumpCover(next, displayCover(createEmptyCoverUiState(), null));
				commit(next);
				return;
			}

			const metadataList = await Promise.all(
				selectedFiles.map((file) => loadMetadataForFile(file)),
			);
			if (editor().hydrateRequestId !== requestId) {
				return;
			}

			if (selectedFiles.length === 1) {
				next = bumpForm(next, populateMetadataFormSingle(metadataList[0] ?? {}));
			} else {
				next = bumpForm(
					next,
					populateMetadataFormMulti(
						metadataList.map((metadata) => metadata ?? {}),
						selectedFiles.length,
					),
				);
			}
			next = bumpCover(
				next,
				refreshCoverFromOwners(jobType, session.fileList, selectedFiles, {
					...next.cover,
					hasCustomCoverArt: false,
					coverArtRemovalRequested: false,
				}),
			);
			commit(next);

			const focused = next.focusedFieldId;
			if (focused) {
				requestAnimationFrame(() => {
					document.getElementById(focused)?.focus();
				});
			}

			await autoLoadCoverIfNeeded(session.fileList, selectedFiles, jobType, requestId);
		},
		setFieldValue(command) {
			const definition = getMetadataFieldDefinitionByInputId(command.inputId);
			if (!definition) return;
			const current = editor();
			const withValue = replaceField(current.form, definition.inputId, { value: command.value });
			commit(bumpForm(current, applyFieldInput(withValue, definition.inputId)));
		},
		setFieldAction(command) {
			const definition = getMetadataFieldDefinitionByActionId(command.actionId);
			if (!definition) return;
			const current = editor();
			commit(bumpForm(current, applyFieldAction(current.form, definition.inputId, command.action)));
		},
		setCoverHovered(hovered) {
			const current = editor();
			commit({ ...current, cover: { ...current.cover, isHovered: hovered } });
		},
		setCoverDragOver(dragOver) {
			const current = editor();
			commit({ ...current, cover: { ...current.cover, isDragOver: dragOver } });
		},
		setCoverUrlInput(value) {
			const current = editor();
			commit({ ...current, cover: { ...current.cover, urlInputValue: value } });
		},
		setCustomCoverArt(coverArtBytes) {
			if (!coverArtBytes || coverArtBytes.length === 0) {
				return;
			}
			applyLoadedCoverArt(coverArtBytes);
		},
		clearCoverArt() {
			const current = editor();
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
			const loadContext = readCoverLoadContext(editor());
			try {
				const selectedFile = await capability().openFile({
					title: 'Select Cover Art Image',
					filters: [{ name: 'Image Files', extensions: [...COVER_ART_IMAGE_EXTENSION_HINTS] }],
				});
				if (!selectedFile) return;
				const imageData = await capability().loadCoverArtFile(selectedFile);
				applyLoadedCoverArt(imageData, loadContext);
			} catch (error) {
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
			const current = editor();
			if (!raw) {
				commit(
					bumpCover(current, { message: { kind: 'error', text: 'Paste an image URL first.' } }),
				);
				return null;
			}
			const parsed = parseCoverArtUrl(raw);
			if (!parsed) {
				commit(bumpCover(editor(), { message: { kind: 'error', text: 'Invalid URL format.' } }));
				return null;
			}
			if (parsed.protocol !== 'https:') {
				commit(
					bumpCover(editor(), {
						message: { kind: 'error', text: 'Only HTTPS URLs are supported.' },
					}),
				);
				return null;
			}
			const normalized = parsed.toString();
			const loadContext = readCoverLoadContext(editor());
			commit(
				bumpCover(editor(), {
					urlInputValue: normalized,
					isLoading: true,
					message: { kind: 'hidden' },
				}),
			);
			try {
				const imageData = await capability().loadCoverArtFromUrl(normalized);
				if (!coverLoadStillValid(loadContext)) {
					commit(bumpCover(editor(), { isLoading: false, message: { kind: 'hidden' } }));
					return null;
				}
				applyLoadedCoverArt(imageData, loadContext);
				commit(
					bumpCover(editor(), {
						isLoading: false,
						message: { kind: 'success', text: 'Cover art loaded from URL.' },
					}),
				);
				scheduleCoverMessageClear();
				return normalized;
			} catch (error) {
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
			const loadContext = readCoverLoadContext(editor());
			try {
				const imageData = await capability().loadCoverArtFile(imageFile);
				applyLoadedCoverArt(imageData, loadContext);
				return true;
			} catch (error) {
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
		applyLookupMetadata(metadata) {
			const current = editor();
			commit(
				bumpForm(
					current,
					applyMetadataToForm(current.form, metadata, { mode: 'single', markDirty: true }),
				),
			);
		},
		applyDraftValidation(validation) {
			const current = editor();
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
				...bumpForm(current, nextForm),
				statusMessage: validation.ok
					? current.statusMessage
					: (validation.errors.first ?? current.statusMessage),
			});
		},
		async stageCurrentSelectionForProcess() {
			const start = editor();
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
						readUncachedMetadataSnapshot(file, (path) => capability().readAudioMetadata(path)),
				});
			} catch {
				return false;
			}
			if (!prepared.ok) {
				applyValidationFailure(prepared.message);
				return false;
			}
			const latest = editor();
			if (
				latest.selectionKey !== captured.selectionKey ||
				latest.formRevision !== captured.formRevision ||
				latest.coverRevision !== captured.coverRevision
			) {
				return false;
			}
			return commitPreparedMetadataDrafts(prepared.prepared);
		},
		async save() {
			const session = deps.input.session();
			const current = editor();
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
			try {
				const prepared = await prepareMetadataDrafts({
					form: editor().form,
					cover: editor().cover,
					selectedFiles: selectedFilesFromSession(session),
					validate: (patch) => capability().validateMetadataIntentPatch(patch),
					readUncachedMetadata: (file) =>
						readUncachedMetadataSnapshot(file, (path) => capability().readAudioMetadata(path)),
				});
				if (!prepared.ok) {
					commit({
						...editor(),
						saveInProgress: false,
						statusMessage: 'Fix metadata validation errors before saving.',
					});
					return;
				}
				commitPreparedMetadataDrafts(prepared.prepared);
				commit(bumpForm(editor(), resetDirtyState(editor().form)));

				const validPaths = new Set(
					(session.fileList?.files ?? []).filter((file) => file.isValid).map((file) => file.path),
				);
				const pendingEntries = getPendingMetadataIntentEntries().filter(([filePath]) =>
					validPaths.has(filePath),
				);
				if (pendingEntries.length === 0) {
					commit({
						...editor(),
						saveInProgress: false,
						statusMessage: 'No pending metadata changes',
					});
					return;
				}
				const result = await capability().saveMetadataBatch(
					pendingEntries.map(([filePath, metadataIntent]) => ({
						filePath,
						metadataPatch: metadataIntent,
					})),
				);
				for (const entry of result.results) {
					if (entry.status === 'success') {
						clearPendingMetadataForFile(entry.filePath);
					} else if (entry.status === 'failed') {
						console.error(
							`Failed metadata save for ${entry.filePath}:`,
							entry.error ?? entry.message,
						);
					}
				}
				const latest = editor();
				commit({
					...latest,
					saveInProgress: false,
					statusMessage: `Metadata save complete: success=${result.summary.succeeded}, failed=${result.summary.failed}, cancelled=${result.summary.cancelled}`,
					cover: {
						...latest.cover,
						hasCustomCoverArt: false,
						coverArtRemovalRequested: false,
					},
				});
			} catch (error) {
				console.error('Failed to save metadata:', error);
				commit({
					...editor(),
					saveInProgress: false,
					statusMessage: 'Save failed - see console',
				});
			}
		},
		readHasDirtyMetadata() {
			const current = editor();
			return hasDirtyMetadataFields(current.form, current.cover);
		},
		readMetadata() {
			const current = editor();
			return readMetadataForm(current.form, {
				coverArtBytes: current.cover.currentCoverArt,
				coverArtRemovalRequested: current.cover.coverArtRemovalRequested,
			});
		},
		reset() {
			if (coverMessageTimeoutId !== null) {
				window.clearTimeout(coverMessageTimeoutId);
				coverMessageTimeoutId = null;
			}
			clearMetadataSession();
			commit(emptyEditor());
		},
	};
}
