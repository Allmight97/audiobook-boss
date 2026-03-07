import { describe, expect, it, beforeEach, vi } from 'vitest';
import {
	METADATA_FIELD_DEFINITIONS,
	setMetadataFormFieldAction,
	setMetadataFormFieldDirty,
	setMetadataFormFieldValue,
	setMetadataFormModeState,
} from '../metadataForm/state.svelte';

let coverArtBytes: number[] | null = null;
let coverRemoval = false;
let hasCustomCoverArt = false;

vi.mock('../coverArt', () => ({
	getCurrentCoverArt: () => coverArtBytes,
	getHasCustomCoverArt: () => hasCustomCoverArt,
	isCoverArtRemovalRequested: () => coverRemoval,
	setCoverArt: () => {},
}));

import { hasDirtyMetadataFields, readMetadataForm } from '../metadataForm';

function resetMetadataFormState(): void {
	setMetadataFormModeState('single');
	for (const field of METADATA_FIELD_DEFINITIONS) {
		setMetadataFormFieldValue(field.inputId, '');
		setMetadataFormFieldAction(field.inputId, 'keep');
		setMetadataFormFieldDirty(field.inputId, false);
	}
}

describe('readMetadataForm (single mode)', () => {
	beforeEach(() => {
		resetMetadataFormState();
		coverArtBytes = null;
		coverRemoval = false;
		hasCustomCoverArt = false;
	});

	it('maps form fields to metadata and includes cover art bytes', () => {
		setMetadataFormFieldValue('meta-title', 'Title');
		setMetadataFormFieldValue('meta-author', 'Author');
		setMetadataFormFieldValue('meta-narrator', 'Narrator');
		setMetadataFormFieldValue('meta-year', '2024');
		setMetadataFormFieldValue('meta-genre', 'Fiction');
		setMetadataFormFieldValue('meta-series', 'Series');
		setMetadataFormFieldValue('meta-series-part', '2');
		setMetadataFormFieldValue('meta-subseries', 'Sub-series');
		setMetadataFormFieldValue('meta-subseries-part', '4');
		setMetadataFormFieldValue('meta-description', 'Desc');
		coverArtBytes = [1, 2, 3];

		const metadata = readMetadataForm({ mode: 'single' });

		expect(metadata).toMatchObject({
			title: 'Title',
			album: 'Title',
			artist: 'Author',
			composer: 'Narrator',
			date: '2024',
			genre: 'Fiction',
			series: 'Series',
			series_part: '2',
			subseries: 'Sub-series',
			subseries_part: '4',
			description: 'Desc',
			cover_art: [1, 2, 3],
		});
	});

	it('accepts YYYY-MM publication date input', () => {
		setMetadataFormFieldValue('meta-year', '2024-07');

		const metadata = readMetadataForm({ mode: 'single' });

		expect(metadata.date).toBe('2024-07');
	});

	it('emits empty cover_art payload when removal requested', () => {
		coverRemoval = true;

		const metadata = readMetadataForm({ mode: 'single' });

		expect(metadata.cover_art).toEqual([]);
	});

	it('includes empty strings for clearable metadata fields', () => {
		const metadata = readMetadataForm({ mode: 'single' });

		expect(metadata.series).toBe('');
		expect(metadata.series_part).toBe('');
		expect(metadata.subseries).toBe('');
		expect(metadata.subseries_part).toBe('');
		expect(metadata.description).toBe('');
	});

	it('treats explicit cover art removal as a dirty metadata change', () => {
		coverRemoval = true;

		expect(hasDirtyMetadataFields()).toBe(true);
	});

	it('treats custom cover art as a dirty metadata change', () => {
		hasCustomCoverArt = true;
		coverArtBytes = [1, 2, 3];

		expect(hasDirtyMetadataFields()).toBe(true);
	});
});

describe('readMetadataForm (multi mode)', () => {
	beforeEach(() => {
		resetMetadataFormState();
		setMetadataFormModeState('multi', 2);
		coverArtBytes = null;
		coverRemoval = false;
		hasCustomCoverArt = false;
	});

	it('uses bulk blank actions for multi-select', () => {
		setMetadataFormFieldAction('meta-year', 'blank');

		const metadata = readMetadataForm({ mode: 'multi', onlyDirty: true });

		expect(metadata.date).toBeUndefined();
	});

	it('applies edited values in multi-select mode', () => {
		setMetadataFormFieldValue('meta-title', 'New Title');
		setMetadataFormFieldDirty('meta-title', true);

		const metadata = readMetadataForm({ mode: 'multi', onlyDirty: true });

		expect(metadata).toMatchObject({
			title: 'New Title',
			album: 'New Title',
		});
	});

	it('ignores cover art changes in multi-select', () => {
		coverArtBytes = [1, 2, 3];
		coverRemoval = true;
		hasCustomCoverArt = true;

		const metadata = readMetadataForm({ mode: 'multi', onlyDirty: true });

		expect(metadata.cover_art).toBeUndefined();
	});
});
