import type { AudiobookMetadata } from '../../types/metadata';
import type {
	FieldAction,
	MetadataField,
	MetadataFormSnapshot,
	SeriesPartWarning,
	SubseriesPartWarning,
} from '../../types/session';

type MetadataFormMode = 'single' | 'multi';
export type MetadataFieldAction = FieldAction;
export type MetadataFieldId =
	| 'meta-title'
	| 'meta-author'
	| 'meta-narrator'
	| 'meta-year'
	| 'meta-genre'
	| 'meta-series'
	| 'meta-series-part'
	| 'meta-subseries'
	| 'meta-subseries-part'
	| 'meta-description';
type MetadataActionId = `${MetadataFieldId}-action`;

type MetadataFieldDefinition = {
	readonly inputId: MetadataFieldId;
	readonly actionId: MetadataActionId;
	/** The engine's name for this field. */
	readonly field: MetadataField;
	/** The tag the output-path preview reads this field as, until that preview moves into the engine. */
	readonly key: keyof AudiobookMetadata;
	readonly mapToAlbum?: boolean;
	readonly placeholder: string;
	readonly label: string;
	readonly span: 1 | 2 | 3 | 4;
	readonly kind: 'input' | 'textarea';
};

/** How each engine field is laid out and labelled in the form. */
export const METADATA_FIELD_DEFINITIONS = [
	{
		inputId: 'meta-title',
		actionId: 'meta-title-action',
		field: 'title',
		key: 'title',
		mapToAlbum: true,
		placeholder: 'Book title',
		label: 'Book Title',
		span: 3,
		kind: 'input',
	},
	{
		inputId: 'meta-year',
		actionId: 'meta-year-action',
		field: 'date',
		key: 'date',
		placeholder: 'YYYY or YYYY-MM',
		label: 'Publication Date',
		span: 1,
		kind: 'input',
	},
	{
		inputId: 'meta-author',
		actionId: 'meta-author-action',
		field: 'author',
		key: 'artist',
		placeholder: 'Author',
		label: 'Author',
		span: 2,
		kind: 'input',
	},
	{
		inputId: 'meta-narrator',
		actionId: 'meta-narrator-action',
		field: 'narrator',
		key: 'composer',
		placeholder: 'Narrator',
		label: 'Narrator',
		span: 2,
		kind: 'input',
	},
	{
		inputId: 'meta-series',
		actionId: 'meta-series-action',
		field: 'series',
		key: 'series',
		placeholder: 'Series name',
		label: 'Series',
		span: 2,
		kind: 'input',
	},
	{
		inputId: 'meta-series-part',
		actionId: 'meta-series-part-action',
		field: 'seriesPart',
		key: 'series_part',
		placeholder: '#',
		label: 'Book #',
		span: 1,
		kind: 'input',
	},
	{
		inputId: 'meta-subseries',
		actionId: 'meta-subseries-action',
		field: 'subseries',
		key: 'subseries',
		placeholder: 'Sub-series name',
		label: 'Sub-series',
		span: 2,
		kind: 'input',
	},
	{
		inputId: 'meta-subseries-part',
		actionId: 'meta-subseries-part-action',
		field: 'subseriesPart',
		key: 'subseries_part',
		placeholder: '#',
		label: 'Sub-series #',
		span: 1,
		kind: 'input',
	},
	{
		inputId: 'meta-genre',
		actionId: 'meta-genre-action',
		field: 'genre',
		key: 'genre',
		placeholder: 'Genre',
		label: 'Genre',
		span: 1,
		kind: 'input',
	},
	{
		inputId: 'meta-description',
		actionId: 'meta-description-action',
		field: 'description',
		key: 'description',
		placeholder: 'Description',
		label: 'Description',
		span: 4,
		kind: 'textarea',
	},
] as const satisfies readonly MetadataFieldDefinition[];

type MetadataFieldState = {
	readonly value: string;
	readonly action: MetadataFieldAction;
	readonly dirty: boolean;
	readonly mixed: boolean;
	readonly placeholder: string;
};

type MetadataWarningState = {
	readonly message: string;
	readonly visible: boolean;
};

export type MetadataFormState = {
	readonly mode: MetadataFormMode;
	readonly selectionCount: number;
	readonly fields: Record<MetadataFieldId, MetadataFieldState>;
	readonly seriesPartWarning: MetadataWarningState;
	readonly subseriesPartWarning: MetadataWarningState;
};

const NO_WARNING: MetadataWarningState = { message: '', visible: false };

function seriesPartWarningText(warning: SeriesPartWarning | null): MetadataWarningState {
	switch (warning?.kind) {
		case undefined:
			return NO_WARNING;
		case 'invalid':
			return { message: warning.message, visible: true };
		case 'matchesSubseriesPart':
			return {
				message:
					'Book # matches sub-series #. Keep them aligned only when both series use the same sequence.',
				visible: true,
			};
		case 'missingBookNumber':
			return {
				message: 'Series detected - add Book # (series sequence) for ABS ordering.',
				visible: true,
			};
	}
}

function subseriesPartWarningText(warning: SubseriesPartWarning | null): MetadataWarningState {
	switch (warning?.kind) {
		case undefined:
			return NO_WARNING;
		case 'invalid':
			return { message: warning.message, visible: true };
		case 'missingNumber':
			return {
				message: 'Sub-series detected - add sub-series # (series sequence) for ABS ordering.',
				visible: true,
			};
	}
}

/**
 * The form as the views render it. `typed` holds text the user has entered
 * that the engine has not confirmed yet; it shows in place of the engine's
 * value so typing never lags.
 */
export function toFormState(
	snapshot: MetadataFormSnapshot,
	typed: ReadonlyMap<MetadataField, string>,
): MetadataFormState {
	const fields = {} as Record<MetadataFieldId, MetadataFieldState>;
	for (const definition of METADATA_FIELD_DEFINITIONS) {
		const reported = snapshot.fields.find((field) => field.field === definition.field);
		const pending = typed.get(definition.field);
		fields[definition.inputId] = {
			value: pending ?? reported?.value ?? '',
			action: reported?.action ?? 'keep',
			dirty: pending !== undefined || (reported?.dirty ?? false),
			mixed: pending === undefined && (reported?.mixed ?? false),
			placeholder: definition.placeholder,
		};
	}
	return {
		mode: snapshot.mode,
		selectionCount: snapshot.selectionCount,
		fields,
		seriesPartWarning: seriesPartWarningText(snapshot.seriesPartWarning),
		subseriesPartWarning: subseriesPartWarningText(snapshot.subseriesPartWarning),
	};
}

export function fieldForInputId(inputId: string): MetadataField | undefined {
	return METADATA_FIELD_DEFINITIONS.find((definition) => definition.inputId === inputId)?.field;
}

export function fieldForActionId(actionId: string): MetadataField | undefined {
	return METADATA_FIELD_DEFINITIONS.find((definition) => definition.actionId === actionId)?.field;
}
