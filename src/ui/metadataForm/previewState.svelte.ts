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

type MetadataFormPreviewState = Record<MetadataFormPreviewKey, string>;

const EMPTY_METADATA_FORM_PREVIEW_STATE: MetadataFormPreviewState = {
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

const INPUT_ID_TO_PREVIEW_KEY: Record<string, MetadataFormPreviewKey> = {
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

export const metadataFormPreviewState = $state<MetadataFormPreviewState>({
	...EMPTY_METADATA_FORM_PREVIEW_STATE,
});

function setMetadataFormPreviewValue(key: MetadataFormPreviewKey, value: string): void {
	metadataFormPreviewState[key] = value.trim();
}

export function setMetadataFormPreviewValueByInputId(inputId: string, value: string): void {
	const key = INPUT_ID_TO_PREVIEW_KEY[inputId];
	if (!key) return;
	setMetadataFormPreviewValue(key, value);
}

export function resetMetadataFormPreviewState(): void {
	for (const [key, value] of Object.entries(EMPTY_METADATA_FORM_PREVIEW_STATE)) {
		metadataFormPreviewState[key as MetadataFormPreviewKey] = value;
	}
}
