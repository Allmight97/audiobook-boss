import type { AudiobookMetadata } from '../../types/metadata';

export type MetadataFormMode = 'single' | 'multi';
export type MetadataFieldAction = 'keep' | 'blank';
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
export type MetadataActionId =
	| 'meta-title-action'
	| 'meta-author-action'
	| 'meta-narrator-action'
	| 'meta-year-action'
	| 'meta-genre-action'
	| 'meta-series-action'
	| 'meta-series-part-action'
	| 'meta-subseries-action'
	| 'meta-subseries-part-action'
	| 'meta-description-action';

export type MetadataFieldDefinition = {
	readonly inputId: MetadataFieldId;
	readonly actionId: MetadataActionId;
	readonly key: keyof AudiobookMetadata;
	readonly placeholder: string;
	readonly label: string;
	readonly span: 1 | 2 | 3 | 4;
	readonly kind: 'input' | 'textarea';
	readonly mapToAlbum?: boolean;
	readonly unconditional?: boolean;
};

export const METADATA_FIELD_DEFINITIONS = [
	{
		inputId: 'meta-title',
		actionId: 'meta-title-action',
		key: 'title',
		placeholder: 'Book title',
		label: 'Book Title',
		span: 3,
		kind: 'input',
		mapToAlbum: true,
	},
	{
		inputId: 'meta-year',
		actionId: 'meta-year-action',
		key: 'date',
		placeholder: 'YYYY or YYYY-MM',
		label: 'Publication Date',
		span: 1,
		kind: 'input',
	},
	{
		inputId: 'meta-author',
		actionId: 'meta-author-action',
		key: 'artist',
		placeholder: 'Author',
		label: 'Author',
		span: 2,
		kind: 'input',
	},
	{
		inputId: 'meta-narrator',
		actionId: 'meta-narrator-action',
		key: 'composer',
		placeholder: 'Narrator',
		label: 'Narrator',
		span: 2,
		kind: 'input',
	},
	{
		inputId: 'meta-series',
		actionId: 'meta-series-action',
		key: 'series',
		placeholder: 'Series name',
		label: 'Series',
		span: 2,
		kind: 'input',
		unconditional: true,
	},
	{
		inputId: 'meta-series-part',
		actionId: 'meta-series-part-action',
		key: 'series_part',
		placeholder: '#',
		label: 'Book #',
		span: 1,
		kind: 'input',
		unconditional: true,
	},
	{
		inputId: 'meta-subseries',
		actionId: 'meta-subseries-action',
		key: 'subseries',
		placeholder: 'Sub-series name',
		label: 'Sub-series',
		span: 2,
		kind: 'input',
		unconditional: true,
	},
	{
		inputId: 'meta-subseries-part',
		actionId: 'meta-subseries-part-action',
		key: 'subseries_part',
		placeholder: '#',
		label: 'Sub-series #',
		span: 1,
		kind: 'input',
		unconditional: true,
	},
	{
		inputId: 'meta-genre',
		actionId: 'meta-genre-action',
		key: 'genre',
		placeholder: 'Genre',
		label: 'Genre',
		span: 1,
		kind: 'input',
	},
	{
		inputId: 'meta-description',
		actionId: 'meta-description-action',
		key: 'description',
		placeholder: 'Description',
		label: 'Description',
		span: 4,
		kind: 'textarea',
		unconditional: true,
	},
] as const satisfies readonly MetadataFieldDefinition[];

export type MetadataFieldState = {
	readonly value: string;
	readonly action: MetadataFieldAction;
	readonly dirty: boolean;
	readonly mixed: boolean;
	readonly placeholder: string;
};

export type MetadataWarningState = {
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

export const EMPTY_WARNING_STATE: MetadataWarningState = {
	message: '',
	visible: false,
};

function createEmptyFieldState(definition: MetadataFieldDefinition): MetadataFieldState {
	return {
		value: '',
		action: 'keep',
		dirty: false,
		mixed: false,
		placeholder: definition.placeholder,
	};
}

export function createEmptyFieldsState(): Record<MetadataFieldId, MetadataFieldState> {
	const fields = {} as Record<MetadataFieldId, MetadataFieldState>;
	for (const definition of METADATA_FIELD_DEFINITIONS) {
		fields[definition.inputId] = createEmptyFieldState(definition);
	}
	return fields;
}

export function createEmptyFormState(): MetadataFormState {
	return {
		mode: 'single',
		selectionCount: 0,
		fields: createEmptyFieldsState(),
		seriesPartWarning: EMPTY_WARNING_STATE,
		subseriesPartWarning: EMPTY_WARNING_STATE,
	};
}

export function getMetadataFieldDefinitionByInputId(
	inputId: string,
): MetadataFieldDefinition | undefined {
	return METADATA_FIELD_DEFINITIONS.find((definition) => definition.inputId === inputId);
}

export function getMetadataFieldDefinitionByActionId(
	actionId: string,
): MetadataFieldDefinition | undefined {
	return METADATA_FIELD_DEFINITIONS.find((definition) => definition.actionId === actionId);
}

export function replaceField(
	form: MetadataFormState,
	inputId: MetadataFieldId,
	patch: Partial<MetadataFieldState>,
): MetadataFormState {
	return {
		...form,
		fields: {
			...form.fields,
			[inputId]: { ...form.fields[inputId], ...patch },
		},
	};
}
