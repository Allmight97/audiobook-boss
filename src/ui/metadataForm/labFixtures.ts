import type { AudiobookMetadata } from '../../types/metadata';
import {
	applyMetadataFormValidationWarnings,
	applyMetadataToForm,
	populateMetadataFormMulti,
	populateMetadataFormSingle,
	type MetadataFormMode,
} from '.';

export type MetadataFormLabPresetId = 'single-clean-populated' | 'multi-mixed-dirty-warning';

export type MetadataFormLabPreset = {
	id: MetadataFormLabPresetId;
	label: string;
	mode: MetadataFormMode;
	summary: string;
	apply: () => void;
};

const singleCleanMetadata: Partial<AudiobookMetadata> = {
	title: 'The Cartographer of Small Things',
	artist: 'Nina Vale',
	composer: 'Rae Okafor',
	date: '2024',
	genre: 'Fiction',
	series: 'Atlas House',
	series_part: '1',
	subseries: '',
	subseries_part: '',
	description: 'A clean single-book fixture with populated fields and no dirty markers.',
};

const multiMixedMetadata: Partial<AudiobookMetadata>[] = [
	{
		title: 'The Shared Title',
		artist: 'Nina Vale',
		composer: 'Rae Okafor',
		date: '2023',
		genre: 'Fiction',
		series: 'Atlas House',
		series_part: '',
		description: 'First selected file description.',
	},
	{
		title: 'The Shared Title',
		artist: 'Mara Wynn',
		composer: 'Dev Patel',
		date: '2024',
		genre: 'Fiction',
		series: 'Atlas House',
		series_part: '',
		description: 'Second selected file description.',
	},
];

export const metadataFormLabPresets: MetadataFormLabPreset[] = [
	{
		id: 'single-clean-populated',
		label: 'Single clean populated',
		mode: 'single',
		summary: 'Single selection with settled metadata and no dirty or warning state.',
		apply: () => {
			populateMetadataFormSingle(singleCleanMetadata);
			applyMetadataFormValidationWarnings(singleCleanMetadata, {});
		},
	},
	{
		id: 'multi-mixed-dirty-warning',
		label: 'Multi mixed dirty warning',
		mode: 'multi',
		summary: 'Three selected files with mixed fields, one edited field, and a series warning.',
		apply: () => {
			populateMetadataFormMulti(multiMixedMetadata, 3);
			applyMetadataToForm(
				{
					title: 'The Shared Title: Revised',
					series: 'Atlas House',
					series_part: '',
				},
				{ mode: 'multi', markDirty: true },
			);
			applyMetadataFormValidationWarnings({ series: 'Atlas House', series_part: '' }, {});
		},
	},
];

export function applyMetadataFormLabPreset(presetId: MetadataFormLabPresetId): void {
	const preset = metadataFormLabPresets.find((candidate) => candidate.id === presetId);
	if (!preset) {
		metadataFormLabPresets[0]?.apply();
		return;
	}
	preset.apply();
}
