import type { AudiobookMetadata } from '../../types/metadata';
import {
	EMPTY_WARNING_STATE,
	METADATA_FIELD_DEFINITIONS,
	createEmptyFormState,
	replaceField,
	type MetadataFieldAction,
	type MetadataFieldId,
	type MetadataFormMode,
	type MetadataFormState,
} from './fields';

export type { MetadataFormState };

export type MetadataFormPreviewKey =
	| 'title'
	| 'author'
	| 'narrator'
	| 'series'
	| 'seriesPart'
	| 'subseries'
	| 'subseriesPart'
	| 'year'
	| 'genre';

export type MetadataFormPreviewValues = Record<MetadataFormPreviewKey, string>;

const INPUT_ID_TO_PREVIEW_KEY: Partial<Record<MetadataFieldId, MetadataFormPreviewKey>> = {
	'meta-title': 'title',
	'meta-author': 'author',
	'meta-narrator': 'narrator',
	'meta-series': 'series',
	'meta-series-part': 'seriesPart',
	'meta-subseries': 'subseries',
	'meta-subseries-part': 'subseriesPart',
	'meta-year': 'year',
	'meta-genre': 'genre',
};

export const EMPTY_METADATA_FORM_PREVIEW_VALUES: MetadataFormPreviewValues = {
	title: '',
	author: '',
	narrator: '',
	series: '',
	seriesPart: '',
	subseries: '',
	subseriesPart: '',
	year: '',
	genre: '',
};

export type MetadataFormValidationWarnings = {
	readonly byField?: {
		readonly series_part?: string;
		readonly subseries_part?: string;
	};
};

function fieldValueFromMetadata(
	metadata: Partial<AudiobookMetadata>,
	key: (typeof METADATA_FIELD_DEFINITIONS)[number]['key'],
): string {
	if (key === 'date') {
		const date = metadata.date;
		return typeof date === 'string' && date.trim() ? date : '';
	}
	const raw = metadata[key];
	return typeof raw === 'string' ? raw : '';
}

export function readMetadataFormPreviewValues(form: MetadataFormState): MetadataFormPreviewValues {
	const preview = { ...EMPTY_METADATA_FORM_PREVIEW_VALUES };
	for (const field of METADATA_FIELD_DEFINITIONS) {
		const key = INPUT_ID_TO_PREVIEW_KEY[field.inputId];
		if (!key) continue;
		preview[key] = form.fields[field.inputId].value.trim();
	}
	return preview;
}

export function resetDirtyState(form: MetadataFormState): MetadataFormState {
	let next = form;
	for (const field of METADATA_FIELD_DEFINITIONS) {
		next = replaceField(next, field.inputId, { dirty: false, action: 'keep' });
	}
	return next;
}

export function populateMetadataFormSingle(
	metadata: Partial<AudiobookMetadata>,
): MetadataFormState {
	let form: MetadataFormState = {
		...createEmptyFormState(),
		mode: 'single',
		selectionCount: 0,
	};
	for (const field of METADATA_FIELD_DEFINITIONS) {
		form = replaceField(form, field.inputId, {
			value: fieldValueFromMetadata(metadata, field.key),
			mixed: false,
			dirty: false,
			action: 'keep',
		});
	}
	return form;
}

export function populateMetadataFormMulti(
	metadataList: ReadonlyArray<Partial<AudiobookMetadata>>,
	selectionCount: number,
): MetadataFormState {
	let form: MetadataFormState = {
		...createEmptyFormState(),
		mode: 'multi',
		selectionCount,
	};
	const hasMetadata = metadataList.length > 0;
	for (const field of METADATA_FIELD_DEFINITIONS) {
		if (!hasMetadata) {
			form = replaceField(form, field.inputId, {
				value: '',
				mixed: false,
				dirty: false,
				action: 'keep',
			});
			continue;
		}
		const values = metadataList.map((metadata) =>
			field.key === 'date'
				? fieldValueFromMetadata(metadata, field.key)
				: fieldValueFromMetadata(metadata, field.key).trim(),
		);
		const uniqueValues = new Set(values);
		if (uniqueValues.size === 1) {
			form = replaceField(form, field.inputId, {
				value: values[0] ?? '',
				mixed: false,
				dirty: false,
				action: 'keep',
			});
			continue;
		}
		form = replaceField(form, field.inputId, {
			value: '',
			mixed: true,
			dirty: false,
			action: 'keep',
		});
	}
	return form;
}

export function applyMetadataToForm(
	form: MetadataFormState,
	metadata: Partial<AudiobookMetadata>,
	options?: { readonly mode?: MetadataFormMode; readonly markDirty?: boolean },
): MetadataFormState {
	const mode = options?.mode ?? form.mode;
	const shouldMarkDirty = options?.markDirty ?? true;
	let next = { ...form, mode };
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
		next = replaceField(next, field.inputId, {
			value,
			mixed: false,
			dirty: shouldMarkDirty ? true : next.fields[field.inputId].dirty,
			action:
				mode === 'multi' ? (value.trim() ? 'keep' : 'blank') : next.fields[field.inputId].action,
		});
	}
	return next;
}

export function applyFieldInput(
	form: MetadataFormState,
	inputId: MetadataFieldId,
): MetadataFormState {
	const value = form.fields[inputId].value;
	let next = replaceField(form, inputId, { dirty: true });
	if (form.mode === 'multi') {
		next = replaceField(next, inputId, { action: value.trim() ? 'keep' : 'blank' });
	}
	return next;
}

export function applyFieldAction(
	form: MetadataFormState,
	inputId: MetadataFieldId,
	action: MetadataFieldAction,
): MetadataFormState {
	let next = replaceField(form, inputId, { action });
	if (action === 'blank') {
		next = replaceField(next, inputId, { value: '', dirty: true });
	}
	return next;
}

function updateSeriesPartWarning(
	form: MetadataFormState,
	metadata: Partial<AudiobookMetadata>,
	seriesPartError: string | null,
): MetadataFormState {
	if (seriesPartError) {
		return { ...form, seriesPartWarning: { message: seriesPartError, visible: true } };
	}
	const seriesValue = metadata.series?.trim() ?? '';
	const seriesPartValue = metadata.series_part?.trim() ?? '';
	const subseriesValue = metadata.subseries?.trim() ?? '';
	const subseriesPartValue = metadata.subseries_part?.trim() ?? '';
	if (
		seriesValue.length > 0 &&
		subseriesValue.length > 0 &&
		seriesPartValue.length > 0 &&
		subseriesPartValue.length > 0 &&
		seriesPartValue === subseriesPartValue
	) {
		return {
			...form,
			seriesPartWarning: {
				message:
					'Book # matches sub-series #. Keep them aligned only when both series use the same sequence.',
				visible: true,
			},
		};
	}
	return {
		...form,
		seriesPartWarning: {
			message: 'Series detected - add Book # (series sequence) for ABS ordering.',
			visible: seriesValue.length > 0 && seriesPartValue.length === 0,
		},
	};
}

function updateSubseriesPartWarning(
	form: MetadataFormState,
	metadata: Partial<AudiobookMetadata>,
	subseriesPartError: string | null,
): MetadataFormState {
	if (subseriesPartError) {
		return { ...form, subseriesPartWarning: { message: subseriesPartError, visible: true } };
	}
	const subseriesValue = metadata.subseries?.trim() ?? '';
	const subseriesPartValue = metadata.subseries_part?.trim() ?? '';
	return {
		...form,
		subseriesPartWarning: {
			message: 'Sub-series detected - add sub-series # (series sequence) for ABS ordering.',
			visible: subseriesValue.length > 0 && subseriesPartValue.length === 0,
		},
	};
}

export function applyMetadataFormValidationWarnings(
	form: MetadataFormState,
	metadata: Partial<AudiobookMetadata>,
	errors: MetadataFormValidationWarnings,
): MetadataFormState {
	return updateSubseriesPartWarning(
		updateSeriesPartWarning(form, metadata, errors.byField?.series_part ?? null),
		metadata,
		errors.byField?.subseries_part ?? null,
	);
}

export function resetMetadataFormWarnings(form: MetadataFormState): MetadataFormState {
	return {
		...form,
		seriesPartWarning: EMPTY_WARNING_STATE,
		subseriesPartWarning: EMPTY_WARNING_STATE,
	};
}

export function hasDirtyMetadataFields(
	form: MetadataFormState,
	cover: { readonly hasCustomCoverArt: boolean; readonly coverArtRemovalRequested: boolean },
): boolean {
	const hasDirtyTextFields = METADATA_FIELD_DEFINITIONS.some(
		(field) => form.fields[field.inputId].dirty,
	);
	return hasDirtyTextFields || cover.coverArtRemovalRequested || cover.hasCustomCoverArt;
}

export function readMetadataForm(
	form: MetadataFormState,
	options?: {
		readonly mode?: MetadataFormMode;
		readonly onlyDirty?: boolean;
		readonly includeCoverArt?: boolean;
		readonly coverArtBytes?: number[] | null;
		readonly coverArtRemovalRequested?: boolean;
	},
): Partial<AudiobookMetadata> {
	const mode = options?.mode ?? form.mode;
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
		const raw = form.fields[field.inputId].value.trim();
		const dirty = form.fields[field.inputId].dirty;

		if (mode === 'multi') {
			const action = form.fields[field.inputId].action;
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

			const hasSharedValue = !form.fields[field.inputId].mixed && raw.length > 0;
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
		if (options?.coverArtRemovalRequested) {
			metadata.cover_art = [];
		} else {
			const coverBytes = options?.coverArtBytes;
			if (coverBytes && coverBytes.length > 0) {
				metadata.cover_art = coverBytes;
			}
		}
	}

	return metadata;
}

export function commitFocusedControlValue(
	form: MetadataFormState,
	activeElement: Element | null,
): { readonly form: MetadataFormState; readonly focusedFieldId: MetadataFieldId | null } {
	if (
		!(activeElement instanceof HTMLInputElement || activeElement instanceof HTMLTextAreaElement)
	) {
		return { form, focusedFieldId: null };
	}
	const definition = METADATA_FIELD_DEFINITIONS.find((field) => field.inputId === activeElement.id);
	if (!definition) {
		return { form, focusedFieldId: null };
	}
	const value = activeElement.value;
	if (value === form.fields[definition.inputId].value) {
		return { form, focusedFieldId: definition.inputId };
	}
	return {
		form: applyFieldInput(replaceField(form, definition.inputId, { value }), definition.inputId),
		focusedFieldId: definition.inputId,
	};
}
