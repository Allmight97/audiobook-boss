import { createEffect, createSignal, untrack, type Accessor } from 'solid-js';
import type {
	MetadataField,
	MetadataStatus,
	OutputEdits,
	SessionMetadata,
} from '../../types/session';
import { coverArtBytesToDataUrl } from '../../lib/media/coverArtDataUrl';
import { toUserMessage } from '../../lib/tauri/appError';
import {
	liveMetadataCapability,
	type MetadataCapability,
} from '../../lib/tauri/capabilities/metadata';
import type { EngineLink } from '../engineLink';
import {
	COVER_ART_IMAGE_EXTENSION_HINTS,
	COVER_ART_IMAGE_EXTENSION_HINT_PATTERN,
	COVER_MESSAGE_MS,
	coverNoticeMessage,
	HIDDEN_COVER_MESSAGE,
	type CoverArtMessage,
	type CoverUiState,
} from './cover';
import {
	fieldForActionId,
	fieldForInputId,
	toFormState,
	type MetadataFieldAction,
	type MetadataFormState,
} from './fields';
import { tagPreviewValues, type TagPreviewValues } from './tags';

export type MetadataView = {
	readonly form: MetadataFormState;
	readonly cover: CoverUiState;
	readonly tags: TagPreviewValues;
	readonly saveInProgress: boolean;
	readonly statusMessage: string;
};

/**
 * The engine owns the metadata form, its drafts, and Save. This owner shows
 * the engine's snapshot and turns what the user does into intents.
 */
export type MetadataOwner = {
	readonly view: Accessor<MetadataView>;
	readonly capability: Accessor<MetadataCapability>;
	setFieldValue(command: { readonly inputId: string; readonly value: string }): void;
	setFieldAction(command: {
		readonly actionId: string;
		readonly action: MetadataFieldAction;
	}): void;
	setCoverHovered(hovered: boolean): void;
	setCoverDragOver(dragOver: boolean): void;
	setCoverUrlInput(value: string): void;
	clearCoverArt(): void;
	loadCoverArtFromPicker(): Promise<void>;
	loadCoverArtFromUrl(rawInput: string): Promise<void>;
	applyCoverArtDrop(paths: ReadonlyArray<string>): Promise<boolean>;
	save(): Promise<void>;
	reset(): void;
};

type MetadataOwnerDeps = {
	readonly link: EngineLink;
	readonly capability?: MetadataCapability;
};

function waitingText(count: number, text: (files: string) => string): string {
	return count > 0 ? ` ${text(count === 1 ? '1 file' : `${count} files`)}` : '';
}

function outputText(outputs: OutputEdits): string {
	const titles = (count: number) => (count === 1 ? '1 export' : `${count} exports`);
	return (
		(outputs.updated > 0 ? ` ${titles(outputs.updated)} updated.` : '') +
		(outputs.elsewhere > 0
			? ` ${titles(outputs.elsewhere)} already finished keep their folder; their tags now name another.`
			: '') +
		(outputs.restartOffered > 0
			? ` ${titles(outputs.restartOffered)} would move; choose whether to restart.`
			: '') +
		(outputs.failed > 0 ? ` ${titles(outputs.failed)} could not take the edit.` : '')
	);
}

/** Words the engine's account of the last metadata action. */
function statusText(status: MetadataStatus | null): string {
	switch (status?.kind) {
		case undefined:
			return '';
		case 'draftInvalid':
			return status.message;
		case 'saveAlreadyInProgress':
			return 'Save already in progress...';
		case 'preparingSave':
			return 'Preparing metadata save...';
		case 'saveInvalid':
			return 'Fix metadata validation errors before saving.';
		case 'noPendingChanges':
			return 'No pending metadata changes';
		case 'groupedEditsKept':
			return 'Title edits saved in this session. Process the title to write them to its merged audiobook.';
		case 'saveComplete':
			return (
				`Metadata save complete: success=${status.succeeded}, failed=${status.failed}, cancelled=${status.cancelled}` +
				waitingText(
					status.waiting,
					(files) => `${files} will be saved when the export reading it finishes.`,
				) +
				waitingText(
					status.held,
					(files) => `${files} from a download was not changed; the edit goes with its exports.`,
				) +
				outputText(status.outputs)
			);
		case 'saveCancelled':
			return 'Metadata save cancelled.';
		case 'saveFailed':
			return toUserMessage(status.error, { fallback: 'Metadata save failed.' });
		case 'deferredWritesFinished':
			return (
				waitingText(
					status.written,
					(files) => `${files} saved after the export reading it finished.`,
				) +
				waitingText(
					status.failed,
					(files) =>
						`${files} could not be saved after the export finished. Save again to retry titles still in the list.`,
				)
			).trim();
	}
}

export function createMetadataOwner(deps: MetadataOwnerDeps): MetadataOwner {
	const { link } = deps;
	const capabilityValue = deps.capability ?? liveMetadataCapability;
	const capability: Accessor<MetadataCapability> = () => capabilityValue;
	const [rev, bump] = createSignal(0, { ownedWrite: true });
	// Text entered and not yet confirmed by the engine, shown so typing never lags.
	// Unconfirmed typing, tied to the form it was typed into.
	const typed = new Map<MetadataField, { readonly value: string; readonly binding: number }>();
	// View-local cover state: none of it is session truth.
	let isHovered = false;
	let isDragOver = false;
	let urlInputValue = '';
	let imageDataUrl: string | null = null;
	let localMessage: CoverArtMessage = HIDDEN_COVER_MESSAGE;
	let hiddenNoticeSerial = 0;
	let messageTimer: ReturnType<typeof setTimeout> | undefined;
	// Advances on reset so a cover fetch from before it is dropped.
	let generation = 0;

	function changed(): void {
		bump((n) => n + 1);
	}

	function hideMessageLater(hide: () => void): void {
		if (messageTimer !== undefined) clearTimeout(messageTimer);
		messageTimer = setTimeout(() => {
			messageTimer = undefined;
			hide();
			changed();
		}, COVER_MESSAGE_MS);
	}

	function showLocalMessage(message: CoverArtMessage): void {
		localMessage = message;
		changed();
		hideMessageLater(() => {
			localMessage = HIDDEN_COVER_MESSAGE;
		});
	}

	// The engine reports that the image changed; the bytes are fetched once per change.
	createEffect(
		() => {
			const cover = link.metadata().cover;
			return `${cover.imageRevision}:${cover.present}`;
		},
		() => {
			const { imageRevision, present } = untrack(() => link.metadata().cover);
			if (!present) {
				// The view already shows no image while the engine reports none.
				imageDataUrl = null;
				return;
			}
			const started = generation;
			void link
				.coverArt()
				.then((bytes) => {
					// A newer image has its own fetch.
					if (
						started !== generation ||
						untrack(() => link.metadata().cover.imageRevision) !== imageRevision
					)
						return;
					imageDataUrl = bytes?.length ? coverArtBytesToDataUrl(bytes) : null;
					changed();
				})
				.catch((error: unknown) => console.error('Failed to fetch cover art:', error));
		},
	);

	// A cover notice hides itself after a moment; a request for a URL stays.
	createEffect(
		() => link.metadata().cover.noticeSerial,
		(serial) => {
			const notice = untrack(() => link.metadata().cover.notice);
			if (!notice || notice.kind === 'urlRequired') return;
			hideMessageLater(() => {
				hiddenNoticeSerial = serial;
			});
		},
	);

	function coverMessage(metadata: SessionMetadata): CoverArtMessage {
		if (localMessage.kind !== 'hidden') return localMessage;
		if (metadata.cover.noticeSerial <= hiddenNoticeSerial) return HIDDEN_COVER_MESSAGE;
		return coverNoticeMessage(metadata.cover.notice);
	}

	const view: Accessor<MetadataView> = () => {
		rev();
		const metadata = link.metadata();
		const form = toFormState(
			metadata.form,
			new Map(
				[...typed]
					.filter(([, entry]) => entry.binding === metadata.binding)
					.map(([field, entry]) => [field, entry.value]),
			),
		);
		return {
			form,
			cover: {
				imageDataUrl: metadata.cover.present ? imageDataUrl : null,
				isLoading: metadata.cover.loading,
				message: coverMessage(metadata),
				isHovered,
				isDragOver,
				urlInputValue,
				hasCustomCoverArt: metadata.cover.custom,
				coverArtRemovalRequested: metadata.cover.removalRequested,
			},
			tags: tagPreviewValues(metadata.tags),
			saveInProgress: metadata.saveInProgress,
			statusMessage: statusText(metadata.status) || (metadata.form.validationMessage ?? ''),
		};
	};

	async function loadCoverFromFile(path: string): Promise<boolean> {
		try {
			return (await link.send({ kind: 'loadCoverFromFile', path })).kind === 'applied';
		} catch (error) {
			showLocalMessage({
				kind: 'error',
				text: toUserMessage(error, { fallback: 'Unable to load cover art.' }),
			});
			return false;
		}
	}

	return {
		view,
		capability,
		setFieldValue(command) {
			const field = fieldForInputId(command.inputId);
			if (!field) return;
			const entry = { value: command.value, binding: link.metadata().binding };
			typed.set(field, entry);
			changed();
			link
				.send({ kind: 'setField', field, value: command.value })
				.catch((error: unknown) => console.error('Failed to record a metadata edit:', error))
				.finally(() => {
					// Later keystrokes keep their own text until their own reply.
					if (typed.get(field) !== entry) return;
					typed.delete(field);
					changed();
				});
		},
		setFieldAction(command) {
			const field = fieldForActionId(command.actionId);
			if (!field) return;
			typed.delete(field);
			changed();
			link.post({ kind: 'setFieldAction', field, action: command.action });
		},
		setCoverHovered(hovered) {
			isHovered = hovered;
			changed();
		},
		setCoverDragOver(dragOver) {
			isDragOver = dragOver;
			changed();
		},
		setCoverUrlInput(value) {
			urlInputValue = value;
			changed();
		},
		clearCoverArt() {
			urlInputValue = '';
			localMessage = HIDDEN_COVER_MESSAGE;
			changed();
			link.post({ kind: 'clearCover' });
		},
		async loadCoverArtFromPicker() {
			let selected: string | null;
			try {
				selected = await capabilityValue.openFile({
					title: 'Select Cover Art Image',
					filters: [{ name: 'Image Files', extensions: [...COVER_ART_IMAGE_EXTENSION_HINTS] }],
				});
			} catch (error) {
				console.error('Failed to open the image picker:', error);
				showLocalMessage({
					kind: 'error',
					text: toUserMessage(error, { fallback: 'Unable to open the image picker.' }),
				});
				return;
			}
			if (selected) await loadCoverFromFile(selected);
		},
		async loadCoverArtFromUrl(rawInput) {
			const url = rawInput.trim();
			if (url) {
				urlInputValue = url;
				changed();
			}
			await link.send({ kind: 'loadCoverFromUrl', url });
		},
		async applyCoverArtDrop(paths) {
			const image = paths.find((path) => COVER_ART_IMAGE_EXTENSION_HINT_PATTERN.test(path));
			return image ? loadCoverFromFile(image) : false;
		},
		async save() {
			await link.send({ kind: 'save' });
		},
		reset() {
			generation += 1;
			if (messageTimer !== undefined) clearTimeout(messageTimer);
			messageTimer = undefined;
			typed.clear();
			isHovered = false;
			isDragOver = false;
			urlInputValue = '';
			imageDataUrl = null;
			localMessage = HIDDEN_COVER_MESSAGE;
			changed();
		},
	};
}
