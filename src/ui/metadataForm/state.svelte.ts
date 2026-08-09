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
	inputId: MetadataFieldId;
	actionId: MetadataActionId;
	key: keyof AudiobookMetadata;
	placeholder: string;
	mapToAlbum?: boolean;
	unconditional?: boolean;
};

export const METADATA_FIELD_DEFINITIONS = [
	{
		inputId: 'meta-title',
		actionId: 'meta-title-action',
		key: 'title',
		placeholder: 'Book title',
		mapToAlbum: true,
	},
	{
		inputId: 'meta-author',
		actionId: 'meta-author-action',
		key: 'artist',
		placeholder: 'Author',
	},
	{
		inputId: 'meta-narrator',
		actionId: 'meta-narrator-action',
		key: 'composer',
		placeholder: 'Narrator',
	},
	{
		inputId: 'meta-year',
		actionId: 'meta-year-action',
		key: 'date',
		placeholder: 'YYYY or YYYY-MM',
	},
	{
		inputId: 'meta-genre',
		actionId: 'meta-genre-action',
		key: 'genre',
		placeholder: 'Genre',
	},
	{
		inputId: 'meta-series',
		actionId: 'meta-series-action',
		key: 'series',
		placeholder: 'Series name',
		unconditional: true,
	},
	{
		inputId: 'meta-series-part',
		actionId: 'meta-series-part-action',
		key: 'series_part',
		placeholder: '#',
		unconditional: true,
	},
	{
		inputId: 'meta-subseries',
		actionId: 'meta-subseries-action',
		key: 'subseries',
		placeholder: 'Sub-series name',
		unconditional: true,
	},
	{
		inputId: 'meta-subseries-part',
		actionId: 'meta-subseries-part-action',
		key: 'subseries_part',
		placeholder: '#',
		unconditional: true,
	},
	{
		inputId: 'meta-description',
		actionId: 'meta-description-action',
		key: 'description',
		placeholder: 'Description',
		unconditional: true,
	},
] as const satisfies readonly MetadataFieldDefinition[];

export type MetadataFieldState = {
	value: string;
	action: MetadataFieldAction;
	dirty: boolean;
	mixed: boolean;
	placeholder: string;
};

type MetadataWarningState = {
	message: string;
	visible: boolean;
};

type MetadataFormState = {
	mode: MetadataFormMode;
	selectionCount: number;
	fields: Record<MetadataFieldId, MetadataFieldState>;
	seriesPartWarning: MetadataWarningState;
	subseriesPartWarning: MetadataWarningState;
};

function createEmptyFieldState(
	definition: MetadataFieldDefinition,
): [MetadataFieldId, MetadataFieldState] {
	return [
		definition.inputId,
		{
			value: '',
			action: 'keep',
			dirty: false,
			mixed: false,
			placeholder: definition.placeholder,
		},
	];
}

function createEmptyFieldsState(): Record<MetadataFieldId, MetadataFieldState> {
	return Object.fromEntries(
		METADATA_FIELD_DEFINITIONS.map((definition) => createEmptyFieldState(definition)),
	) as Record<MetadataFieldId, MetadataFieldState>;
}

const EMPTY_WARNING_STATE: MetadataWarningState = {
	message: '',
	visible: false,
};

export const metadataFormState = $state<MetadataFormState>({
	mode: 'single',
	selectionCount: 0,
	fields: createEmptyFieldsState(),
	seriesPartWarning: { ...EMPTY_WARNING_STATE },
	subseriesPartWarning: { ...EMPTY_WARNING_STATE },
});

// Monotonic owner revision for async prepare -> commit guards. Increment for
// every form-state mutation, including programmatic selection hydration, so an
// older validation result can never reset or commit over a newer visible form.
let metadataFormRevision = 0;

function bumpMetadataFormRevision(): void {
	metadataFormRevision += 1;
}

export function getMetadataFormRevision(): number {
	return metadataFormRevision;
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

export function setMetadataFormFieldValue(inputId: MetadataFieldId, value: string): void {
	metadataFormState.fields[inputId].value = value;
	bumpMetadataFormRevision();
}

export function setMetadataFormFieldAction(
	inputId: MetadataFieldId,
	action: MetadataFieldAction,
): void {
	metadataFormState.fields[inputId].action = action;
	bumpMetadataFormRevision();
}

export function setMetadataFormFieldDirty(inputId: MetadataFieldId, dirty: boolean): void {
	metadataFormState.fields[inputId].dirty = dirty;
	bumpMetadataFormRevision();
}

export function setMetadataFormFieldMixed(inputId: MetadataFieldId, mixed: boolean): void {
	metadataFormState.fields[inputId].mixed = mixed;
	bumpMetadataFormRevision();
}

export function setMetadataFormModeState(mode: MetadataFormMode, selectionCount: number = 0): void {
	metadataFormState.mode = mode;
	metadataFormState.selectionCount = mode === 'multi' ? selectionCount : 0;
	bumpMetadataFormRevision();
}

export function setSeriesPartWarning(message: string, visible: boolean): void {
	metadataFormState.seriesPartWarning.message = message;
	metadataFormState.seriesPartWarning.visible = visible;
}

export function setSubseriesPartWarning(message: string, visible: boolean): void {
	metadataFormState.subseriesPartWarning.message = message;
	metadataFormState.subseriesPartWarning.visible = visible;
}

export function resetMetadataFormWarnings(): void {
	metadataFormState.seriesPartWarning = { ...EMPTY_WARNING_STATE };
	metadataFormState.subseriesPartWarning = { ...EMPTY_WARNING_STATE };
}
