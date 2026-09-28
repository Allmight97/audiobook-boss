import type { AudiobookMetadata } from '../../types/metadata';
import type { MetadataIntentPatch } from '../../types/metadataIntent';
import { buildMetadataDraftIntent } from './draft';
import {
	METADATA_FIELD_DEFINITIONS,
	createEmptyFormState,
	replaceField,
	type MetadataFieldAction,
	type MetadataFieldId,
	type MetadataFormState,
} from './fields';

export type { MetadataFormState };

type MetadataFormValidationWarnings = {
	readonly byField?: {
		readonly series_part?: string;
		readonly subseries_part?: string;
	};
};

function fieldValueFromMetadata(
	metadata: Partial<AudiobookMetadata>,
	key: (typeof METADATA_FIELD_DEFINITIONS)[number]['key'],
): string {
	const raw = metadata[key];
	if (key === 'date') {
		return typeof raw === 'string' && raw.trim() ? raw : '';
	}
	return typeof raw === 'string' ? raw : '';
}

function hydratedField(value: string, mixed: boolean) {
	return { value, mixed, dirty: false, action: 'keep' as const, hydrated: { value, mixed } };
}

export function formValue(form: MetadataFormState, inputId: MetadataFieldId): string {
	return form.fields[inputId].value.trim();
}

/** Accepts the current values as the new baseline once their edits are staged. */
export function resetDirtyState(form: MetadataFormState): MetadataFormState {
	let next = form;
	for (const field of METADATA_FIELD_DEFINITIONS) {
		const state = next.fields[field.inputId];
		next = replaceField(next, field.inputId, hydratedField(state.value, state.mixed));
	}
	return next;
}

export function populateMetadataFormSingle(
	metadata: Partial<AudiobookMetadata>,
): MetadataFormState {
	let form: MetadataFormState = { ...createEmptyFormState(), mode: 'single', selectionCount: 0 };
	for (const field of METADATA_FIELD_DEFINITIONS) {
		form = replaceField(
			form,
			field.inputId,
			hydratedField(fieldValueFromMetadata(metadata, field.key), false),
		);
	}
	return form;
}

export function populateMetadataFormMulti(
	metadataList: ReadonlyArray<Partial<AudiobookMetadata>>,
	selectionCount: number,
): MetadataFormState {
	let form: MetadataFormState = { ...createEmptyFormState(), mode: 'multi', selectionCount };
	for (const field of METADATA_FIELD_DEFINITIONS) {
		const values = new Set(
			metadataList.map((metadata) => {
				const value = fieldValueFromMetadata(metadata, field.key);
				return field.key === 'date' ? value : value.trim();
			}),
		);
		const shared = values.size <= 1;
		const value = shared ? ([...values][0] ?? '') : '';
		form = replaceField(form, field.inputId, hydratedField(value, !shared));
	}
	return form;
}

/** Applies Lookup's values to a single-title form as explicit edits. */
export function applyLookupValues(
	form: MetadataFormState,
	metadata: Partial<AudiobookMetadata>,
): MetadataFormState {
	let next = form;
	for (const field of METADATA_FIELD_DEFINITIONS) {
		const raw = metadata[field.key];
		if (typeof raw !== 'string') continue;
		const value = field.key === 'date' ? raw.trim() : raw;
		next = replaceField(next, field.inputId, { value, mixed: false, dirty: true });
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

/** Blank clears the field on every selected title; Keep revokes that pending edit. */
export function applyFieldAction(
	form: MetadataFormState,
	inputId: MetadataFieldId,
	action: MetadataFieldAction,
): MetadataFormState {
	const field = form.fields[inputId];
	if (field.action === action) return form;
	if (action === 'blank') {
		return replaceField(form, inputId, { action, value: '', dirty: true });
	}
	return replaceField(form, inputId, { action, ...field.hydrated, dirty: false });
}

function seriesPartWarning(form: MetadataFormState, error: string | null) {
	if (error) return { message: error, visible: true };
	const series = formValue(form, 'meta-series');
	const seriesPart = formValue(form, 'meta-series-part');
	const subseriesPart = formValue(form, 'meta-subseries-part');
	if (series && formValue(form, 'meta-subseries') && seriesPart && seriesPart === subseriesPart) {
		return {
			message:
				'Book # matches sub-series #. Keep them aligned only when both series use the same sequence.',
			visible: true,
		};
	}
	return {
		message: 'Series detected - add Book # (series sequence) for ABS ordering.',
		visible: series.length > 0 && seriesPart.length === 0,
	};
}

function subseriesPartWarning(form: MetadataFormState, error: string | null) {
	if (error) return { message: error, visible: true };
	return {
		message: 'Sub-series detected - add sub-series # (series sequence) for ABS ordering.',
		visible:
			formValue(form, 'meta-subseries').length > 0 &&
			formValue(form, 'meta-subseries-part').length === 0,
	};
}

export function applyMetadataFormValidationWarnings(
	form: MetadataFormState,
	errors: MetadataFormValidationWarnings,
): MetadataFormState {
	return {
		...form,
		seriesPartWarning: seriesPartWarning(form, errors.byField?.series_part ?? null),
		subseriesPartWarning: subseriesPartWarning(form, errors.byField?.subseries_part ?? null),
	};
}

export function hasDirtyFields(form: MetadataFormState): boolean {
	return METADATA_FIELD_DEFINITIONS.some((field) => form.fields[field.inputId].dirty);
}

/**
 * The edit intent the form carries: only fields the user changed (including
 * explicit Blank) become set/clear operations; everything else stays absent so
 * inherited values are neither rewritten nor revalidated. Cover intent is
 * staged by the cover actions themselves.
 */
export function composeFormIntent(form: MetadataFormState): MetadataIntentPatch {
	const draft: Partial<Record<keyof AudiobookMetadata, string>> = {};
	for (const field of METADATA_FIELD_DEFINITIONS) {
		const state = form.fields[field.inputId];
		const blank = state.action === 'blank';
		if (!state.dirty && !blank) continue;
		const value = blank ? '' : state.value.trim();
		draft[field.key] = value;
		if ('mapToAlbum' in field && field.mapToAlbum) draft.album = value;
	}
	return buildMetadataDraftIntent(draft as Partial<AudiobookMetadata>);
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
