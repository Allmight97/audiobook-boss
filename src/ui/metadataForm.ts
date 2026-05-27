import type { AudiobookMetadata } from '../types/metadata';
import {
	getCurrentCoverArt,
	getHasCustomCoverArt,
	isCoverArtRemovalRequested,
	setCoverArt,
} from './coverArt';
import {
	resetMetadataFormPreviewState,
	setMetadataFormPreviewValueByInputId,
} from './metadataForm/previewState.svelte';
import {
	getMetadataFieldDefinitionByActionId,
	getMetadataFieldDefinitionByInputId,
	metadataFormState,
	METADATA_FIELD_DEFINITIONS,
	setMetadataFormFieldAction,
	setMetadataFormFieldDirty,
	setMetadataFormFieldMixed,
	setMetadataFormFieldValue,
	setMetadataFormModeState,
	resetMetadataFormWarnings,
	type MetadataActionId,
	type MetadataFieldId,
	type MetadataFieldAction,
	type MetadataFormMode,
} from './metadataForm/state.svelte';
import { updateTagPreview } from './tagPreview';

function readFieldValue(inputId: MetadataFieldId): string {
	return metadataFormState.fields[inputId].value;
}

function readFieldAction(inputId: MetadataFieldId): MetadataFieldAction {
	return metadataFormState.fields[inputId].action;
}

function markDirty(inputId: MetadataFieldId): void {
	setMetadataFormFieldDirty(inputId, true);
}

function isDirty(inputId: MetadataFieldId): boolean {
	return metadataFormState.fields[inputId].dirty;
}

export function setMetadataFormMode(mode: MetadataFormMode, selectionCount?: number): void {
	setMetadataFormModeState(mode, selectionCount);
}

export function initMetadataFormEvents(): void {
	resetMetadataFormPreviewState();
	for (const field of METADATA_FIELD_DEFINITIONS) {
		setMetadataFormPreviewValueByInputId(
			field.inputId,
			metadataFormState.fields[field.inputId].value,
		);
	}
	updateTagPreview();
}

export function onMetadataFormFieldInput(inputId: string): void {
	const field = getMetadataFieldDefinitionByInputId(inputId);
	if (!field) return;

	const value = readFieldValue(field.inputId);
	setMetadataFormFieldValue(field.inputId, value);
	markDirty(field.inputId);
	setMetadataFormPreviewValueByInputId(field.inputId, value);

	if (metadataFormState.mode === 'multi') {
		const actionValue: MetadataFieldAction = value.trim() ? 'keep' : 'blank';
		setMetadataFormFieldAction(field.inputId, actionValue);
	}

	updateTagPreview();
}

export function onMetadataFormActionSelectChange(actionId: string): void {
	const field = getMetadataFieldDefinitionByActionId(actionId);
	if (!field) return;

	const nextAction = readFieldAction(field.inputId);
	setMetadataFormFieldAction(field.inputId, nextAction);

	if (metadataFormState.fields[field.inputId].action === 'blank') {
		setMetadataFormFieldValue(field.inputId, '');
		markDirty(field.inputId);
		setMetadataFormPreviewValueByInputId(field.inputId, '');
	}

	updateTagPreview();
}

export function resetDirtyState(): void {
	for (const field of METADATA_FIELD_DEFINITIONS) {
		setMetadataFormFieldDirty(field.inputId, false);
		setMetadataFormFieldAction(field.inputId, 'keep');
	}
}

export function hasDirtyMetadataFields(): boolean {
	const hasDirtyTextFields = METADATA_FIELD_DEFINITIONS.some((field) =>
		Boolean(metadataFormState.fields[field.inputId].dirty),
	);
	return hasDirtyTextFields || isCoverArtRemovalRequested() || getHasCustomCoverArt();
}

export function populateMetadataFormSingle(metadata: Partial<AudiobookMetadata>): void {
	setMetadataFormMode('single');
	resetMetadataFormWarnings();

	for (const field of METADATA_FIELD_DEFINITIONS) {
		let value = '';
		if (field.key === 'date') {
			const date = metadata.date;
			if (typeof date === 'string' && date.trim()) {
				value = date;
			}
		} else {
			const raw = metadata[field.key];
			if (typeof raw === 'string') {
				value = raw;
			}
		}

		setMetadataFormFieldValue(field.inputId, value);
		setMetadataFormFieldMixed(field.inputId, false);
		setMetadataFormPreviewValueByInputId(field.inputId, value);
	}

	if (!getHasCustomCoverArt()) {
		setCoverArt(metadata.cover_art || null);
	}

	resetDirtyState();
	updateTagPreview();
}

export function populateMetadataFormMulti(
	metadataList: Partial<AudiobookMetadata>[],
	selectionCount: number,
): void {
	setMetadataFormMode('multi', selectionCount);
	resetMetadataFormWarnings();

	const hasMetadata = metadataList.length > 0;

	for (const field of METADATA_FIELD_DEFINITIONS) {
		if (!hasMetadata) {
			setMetadataFormFieldValue(field.inputId, '');
			setMetadataFormFieldMixed(field.inputId, false);
			setMetadataFormPreviewValueByInputId(field.inputId, '');
			continue;
		}

		const values = metadataList.map((metadata) => {
			if (field.key === 'date') {
				const date = metadata.date;
				return typeof date === 'string' && date.trim() ? date : '';
			}
			const raw = metadata[field.key];
			return typeof raw === 'string' ? raw.trim() : '';
		});

		const uniqueValues = new Set(values);
		if (uniqueValues.size === 1) {
			const value = values[0] ?? '';
			setMetadataFormFieldValue(field.inputId, value);
			setMetadataFormFieldMixed(field.inputId, false);
			setMetadataFormPreviewValueByInputId(field.inputId, value);
			continue;
		}

		setMetadataFormFieldValue(field.inputId, '');
		setMetadataFormFieldMixed(field.inputId, true);
		setMetadataFormPreviewValueByInputId(field.inputId, '');
	}

	if (!getHasCustomCoverArt()) {
		setCoverArt(null);
	}

	resetDirtyState();
	updateTagPreview();
}

export function applyMetadataToForm(
	metadata: Partial<AudiobookMetadata>,
	options?: { mode?: MetadataFormMode; markDirty?: boolean },
): void {
	const mode = options?.mode ?? 'single';
	const shouldMarkDirty = options?.markDirty ?? true;

	for (const field of METADATA_FIELD_DEFINITIONS) {
		let value: string | null = null;
		if (field.key === 'date') {
			const date = metadata.date;
			if (typeof date === 'string') {
				value = date.trim();
			}
		} else {
			const raw = metadata[field.key];
			if (typeof raw === 'string') {
				value = raw;
			}
		}
		if (value === null) continue;

		setMetadataFormFieldValue(field.inputId, value);
		setMetadataFormFieldMixed(field.inputId, false);
		setMetadataFormPreviewValueByInputId(field.inputId, value);
		if (shouldMarkDirty) {
			setMetadataFormFieldDirty(field.inputId, true);
		}
		if (mode === 'multi') {
			setMetadataFormFieldAction(field.inputId, value.trim() ? 'keep' : 'blank');
		}
	}

	updateTagPreview();
}

export function readMetadataForm(options?: {
	mode?: MetadataFormMode;
	onlyDirty?: boolean;
	includeCoverArt?: boolean;
}): Partial<AudiobookMetadata> {
	const mode = options?.mode ?? 'single';
	const onlyDirty = options?.onlyDirty ?? false;
	const includeCoverArt = options?.includeCoverArt ?? true;
	const metadata: Partial<AudiobookMetadata> = {};
	const setMetadataValue = <K extends keyof AudiobookMetadata>(
		key: K,
		value: AudiobookMetadata[K],
	): void => {
		metadata[key] = value;
	};

	for (const field of METADATA_FIELD_DEFINITIONS) {
		const raw = readFieldValue(field.inputId).trim();
		const dirty = isDirty(field.inputId);

		if (mode === 'multi') {
			const action = readFieldAction(field.inputId);
			if (action === 'blank') {
				if (field.key === 'date') {
					setMetadataValue(field.key, undefined as AudiobookMetadata[typeof field.key]);
				} else {
					setMetadataValue(field.key, '' as AudiobookMetadata[typeof field.key]);
					if ('mapToAlbum' in field && field.mapToAlbum && field.key === 'title') {
						metadata.album = '';
					}
				}
				continue;
			}

			const hasSharedValue = !metadataFormState.fields[field.inputId].mixed && raw.length > 0;
			if (!dirty && (onlyDirty || !hasSharedValue)) continue;

			if (field.key === 'date') {
				if (!raw) {
					setMetadataValue(field.key, undefined as AudiobookMetadata[typeof field.key]);
					continue;
				}
				setMetadataValue(field.key, raw as AudiobookMetadata[typeof field.key]);
				continue;
			}

			setMetadataValue(field.key, raw as AudiobookMetadata[typeof field.key]);
			if ('mapToAlbum' in field && field.mapToAlbum && field.key === 'title') {
				metadata.album = raw;
			}
			continue;
		}

		if (onlyDirty && !dirty) continue;

		if (field.key === 'date') {
			if (raw) {
				setMetadataValue(field.key, raw as AudiobookMetadata[typeof field.key]);
			} else if (dirty) {
				setMetadataValue(field.key, undefined as AudiobookMetadata[typeof field.key]);
			}
			continue;
		}

		const shouldInclude =
			raw || dirty || ('unconditional' in field && field.unconditional && !onlyDirty);
		if (!shouldInclude) continue;

		setMetadataValue(field.key, raw as AudiobookMetadata[typeof field.key]);
		if ('mapToAlbum' in field && field.mapToAlbum && field.key === 'title') {
			metadata.album = raw;
		}
	}

	if (mode === 'single' && includeCoverArt) {
		if (isCoverArtRemovalRequested()) {
			metadata.cover_art = [];
		} else {
			const coverBytes = getCurrentCoverArt();
			if (coverBytes && coverBytes.length > 0) {
				metadata.cover_art = coverBytes;
			}
		}
	}

	return metadata;
}

export type { MetadataFormMode, MetadataFieldAction, MetadataActionId, MetadataFieldId };
